//! A web page drawn by the system's web view where GPUI lays this element
//! out. The browser card puts one in its body, the device panel one per
//! device tab and generative UI one per expanded HTML surface (sandboxed).
//! The native view sits above everything GPUI draws, so it hides while a
//! screen or a dialog covers the window, and it only exists while the
//! surface is shown: a hidden page costs no web process.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gentle_core::browser::Profile;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{NativeFailure, PUMP_EVERY, PUMP_IDLE, Sandbox, native, page_background};
use crate::shared::ui;

thread_local! {
    /// The GTK pump every live native view shares (see [`shared_pump`]).
    static PUMP: RefCell<Weak<Task<()>>> = const { RefCell::new(Weak::new()) };
}

/// A share of the one GTK pump: the first live view starts it, dropping the
/// last share stops it. One loop serves every view, so its cost doesn't
/// grow with the cards and device tabs open.
fn shared_pump(cx: &App) -> Rc<Task<()>> {
    PUMP.with(|pump| {
        if let Some(task) = pump.borrow().upgrade() {
            return task;
        }
        let task = Rc::new(cx.spawn(async move |cx| {
            loop {
                let every = if cx.update(|cx| cx.active_window().is_some()) {
                    PUMP_EVERY
                } else {
                    PUMP_IDLE
                };
                cx.background_executor().timer(every).await;
                crate::shared::diagnostics::tick();
                native::pump();
            }
        }));
        *pump.borrow_mut() = Rc::downgrade(&task);
        task
    })
}

pub struct WebSurface {
    url: String,
    /// Runs before the page's own scripts, on every load.
    script: Option<String>,
    /// Set for a page the app serves itself, locked down (see [`Sandbox`]).
    sandbox: Option<Sandbox>,
    /// Where the page keeps its cookies and storage.
    profile: Profile,
    /// An agent drives it: out of sight it keeps its native view (and its
    /// page) instead of dropping them.
    live: bool,
    /// A screen covers the window: the native view must not show over it.
    covered: bool,
    /// Not on screen (the card is in the tray, the tab isn't active): no
    /// native view exists, unless it is live.
    hidden: bool,
    /// The native web view, created after the surface is first laid out.
    view: Option<native::NativeView>,
    /// The view is being built (see `place`).
    creating: bool,
    failed: Option<NativeFailure>,
    /// This view's share of the GTK pump while a native view exists (Linux).
    _pump: Option<Rc<Task<()>>>,
    /// Re-places the view when the window moves (Windows hosts it in a popup
    /// positioned in screen coordinates).
    _window_moves: Option<Subscription>,
    /// New-window requests (`target="_blank"`) load here; see
    /// [`Self::follow_popups`].
    popups: Option<(super::Popups, Task<()>)>,
    /// Where Esc the page didn't use goes; see [`Self::relay_escape`].
    escapes: Option<super::Escapes>,
}

/// The size a page out of sight is laid out at (an agent's parked page).
const PARKED: (f32, f32) = (1280., 800.);

impl WebSurface {
    pub fn new(url: String, script: Option<String>) -> Self {
        Self {
            url,
            script,
            sandbox: None,
            profile: Profile::Shared,
            live: false,
            covered: false,
            hidden: false,
            view: None,
            creating: false,
            failed: None,
            _pump: None,
            _window_moves: None,
            popups: None,
            escapes: None,
        }
    }

    /// Load what the page asks to open in a new window (`target="_blank"`,
    /// `window.open`) here instead: a browser card has no other window, and
    /// an agent following a link must land somewhere it can see. Takes
    /// effect on the next native view.
    pub fn follow_popups(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = async_channel::bounded::<String>(8);
        let task = cx.spawn(async move |this, cx| {
            while let Ok(url) = rx.recv().await {
                if this.update(cx, |s, cx| s.load(url, cx)).is_err() {
                    return;
                }
            }
        });
        self.popups = Some((tx, task));
    }

    /// Send Esc the page didn't use to `escapes` (the browser card restores
    /// itself): the web view keeps the keyboard, so it never reaches the
    /// app otherwise. Takes effect on the next native view.
    pub fn relay_escape(&mut self, escapes: super::Escapes) {
        self.escapes = Some(escapes);
    }

    /// The page has the keyboard (it was clicked).
    pub fn has_keyboard(&self) -> bool {
        self.view.as_ref().is_some_and(|view| view.has_keyboard())
    }

    /// An isolated page served by `sandbox`, starting at `url` (one of its
    /// [`super::sandbox_url`]s).
    pub fn sandboxed(url: String, sandbox: Sandbox) -> Self {
        Self {
            sandbox: Some(sandbox),
            ..Self::new(url, None)
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Keep the page's data in `profile`: a different one starts the page
    /// again in that store.
    pub fn set_profile(&mut self, profile: Profile, cx: &mut Context<Self>) {
        if self.profile != profile {
            self.profile = profile;
            self.view = None;
            self._pump = None;
            self.failed = None;
            cx.notify();
        }
    }

    /// While live, hiding keeps the page running out of sight.
    pub fn set_live(&mut self, live: bool, cx: &mut Context<Self>) {
        self.live = live;
        if !live && self.hidden {
            self.hide(cx);
        }
    }

    /// The page runs: its native view exists.
    pub fn is_running(&self) -> bool {
        self.view.is_some()
    }

    /// Make sure the page runs, even out of sight (an agent drives it): a
    /// view is built at [`PARKED`] size when none exists.
    pub fn ensure_running(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view.is_some() || self.creating || self.failed.is_some() {
            return;
        }
        let (w, h) = PARKED;
        self.build(
            Bounds::new(point(px(0.), px(0.)), size(px(w), px(h))),
            false,
            window,
            cx,
        );
    }

    /// Why the page can't run, if it can't.
    pub fn failure(&self) -> Option<String> {
        self.failed
            .as_ref()
            .map(|error| error.render_in(gentle_core::i18n::Language::English))
    }

    /// Run `script`; `done` gets its value as JSON. False when no view runs.
    pub fn evaluate_with(&self, script: &str, done: impl Fn(String) + Send + 'static) -> bool {
        match &self.view {
            Some(view) => {
                view.evaluate_with(script, done);
                true
            }
            None => false,
        }
    }

    /// A PNG of the page; false when no view runs.
    pub fn screenshot(&self, done: super::Shot) -> bool {
        match &self.view {
            Some(view) => {
                view.screenshot(done);
                true
            }
            None => false,
        }
    }

    /// Forget the profile's website data (signs the page out).
    pub fn clear_data(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = &self.view {
            view.clear_data();
            view.reload();
        }
        cx.notify();
    }

    /// Run `script` in the page, if its view exists (nothing otherwise: a
    /// view built later loads the page afresh).
    pub fn evaluate(&self, script: &str) {
        if let Some(view) = &self.view {
            view.evaluate(script);
        }
    }

    /// Show `url`, or reload when it is already showing.
    pub fn navigate(&mut self, url: String, cx: &mut Context<Self>) {
        if let Some(view) = &self.view {
            if url == self.url {
                view.reload();
            } else {
                view.load_url(&url);
            }
        }
        self.url = url;
        cx.notify();
    }

    /// Load `url` even when it is the address showing (an agent's `goto`
    /// after a form posted there, a link opening in a new window): a
    /// fresh GET, not a reload.
    pub fn load(&mut self, url: String, cx: &mut Context<Self>) {
        if let Some(view) = &self.view {
            view.load_url(&url);
        }
        self.url = url;
        cx.notify();
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = &self.view {
            view.reload();
        }
        cx.notify();
    }

    /// Something opened or closed over the page (a screen, a dialog), or
    /// its element stopped being drawn (another tab). Covering hides the
    /// native view right away, without waiting for a frame that may never
    /// draw this element; uncovering shows it at the next placement.
    pub fn set_covered(&mut self, covered: bool, cx: &mut Context<Self>) {
        if self.covered != covered {
            self.covered = covered;
            if covered && let Some(view) = &mut self.view {
                view.conceal();
            }
            cx.notify();
        }
    }

    /// Off screen: drop the native view, and the web process behind it,
    /// unless an agent drives the page (then it only hides). Showing it again
    /// builds a fresh view on the current URL.
    pub fn hide(&mut self, cx: &mut Context<Self>) {
        self.hidden = true;
        if self.live {
            if let Some(view) = &mut self.view {
                view.conceal();
            }
        } else {
            self.view = None;
            self._pump = None;
        }
        cx.notify();
    }

    pub fn show(&mut self, cx: &mut Context<Self>) {
        self.hidden = false;
        cx.notify();
    }

    /// Place the native view over this element. The first call builds it in
    /// a task instead: that runs outside GPUI's update, which matters because
    /// WebView2 pumps the thread's messages while it starts and GPUI's window
    /// procedure would re-enter a borrowed window and abort the app.
    fn place(&mut self, bounds: Bounds<Pixels>, window: &Window, cx: &mut Context<Self>) {
        if self.failed.is_some() || self.creating || self.hidden {
            return;
        }
        if let Some(view) = &mut self.view {
            view.place(window, bounds, !self.covered);
            return;
        }
        self.build(bounds, true, window, cx);
    }

    /// Build the native view in a task, then place it at `bounds` (shown when
    /// `shown`, the next frame places a shown one anyway).
    fn build(
        &mut self,
        bounds: Bounds<Pixels>,
        shown: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let host = match native::host(window) {
            Ok(host) => host,
            Err(error) => {
                self.failed = Some(error);
                cx.notify();
                return;
            }
        };
        self.creating = true;
        let url = self.url.clone();
        let script = self.script.clone();
        let sandbox = self.sandbox.clone();
        let profile = self.profile.clone();
        let relays = super::Relays {
            popups: self.popups.as_ref().map(|(tx, _)| tx.clone()),
            escapes: self.escapes.clone(),
        };
        let background = page_background();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let view = native::NativeView::new(
                host,
                &url,
                background,
                script.as_deref(),
                sandbox.as_ref(),
                &profile,
                relays,
            );
            // A surface dropped meanwhile drops the view, and its popup with
            // it; one hidden meanwhile (and not driven) drops it right here.
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |surface, cx| {
                    surface.creating = false;
                    if surface.hidden && !surface.live {
                        return;
                    }
                    match view {
                        Ok(mut view) => {
                            // The page moved on while the view was starting.
                            if surface.url != url {
                                view.load_url(&surface.url);
                            }
                            // A parked page gets its size right away; a shown
                            // one is placed by the next frame.
                            if !shown {
                                view.place(window, bounds, false);
                            }
                            surface.view = Some(view);
                            // GTK's loop only needs pumping while a view
                            // exists; `hide` drops the pump with the view.
                            if native::NEEDS_PUMP {
                                surface._pump = Some(shared_pump(cx));
                            }
                        }
                        Err(error) => surface.failed = Some(error),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

impl Render for WebSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self._window_moves.is_none() {
            self._window_moves = Some(cx.observe_window_bounds(window, |_, _, cx| cx.notify()));
        }
        let this = cx.entity();
        let note = self.failed.as_ref().map(NativeFailure::render);
        div()
            .size_full()
            .relative()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        this.update(cx, |surface, cx| surface.place(bounds, window, cx));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .when_some(note, |d, note| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_4()
                        .text_size(rems(12. / 16.))
                        .text_color(ui::muted())
                        .child(note),
                )
            })
    }
}
