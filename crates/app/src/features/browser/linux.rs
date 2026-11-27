//! The in-app browser on Linux: WebKitGTK as a child of the app's X11 window.
//! GPUI hands out an XCB handle and `wry` takes an Xlib one; both name the
//! same X window. GTK must be initialized once and its loop pumped while a
//! web view exists (one task shared by every view calls `pump`). Wayland has
//! no child windows, so there the canvas stays in the browser Gentle Shell
//! opened.

use std::ffi::c_ulong;

use gentle_core::browser::Profile;
use std::sync::LazyLock;

use gpui_kit::{Bounds, Pixels, Window};
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle,
};

/// GTK's loop runs only when someone pumps it.
pub(super) const NEEDS_PUMP: bool = true;

/// Run GTK's pending events (WebKitGTK's drawing and input). Only called on
/// the main thread while a web view exists.
pub(super) fn pump() {
    if GTK.is_ok() {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
    }
}

/// GTK, initialized on first use (always from the main thread).
static GTK: LazyLock<Result<(), super::NativeFailure>> = LazyLock::new(|| {
    gtk::init().map_err(|error| {
        super::NativeFailure::with_error("The browser could not start GTK: {error}", error)
    })
});

/// The app's X window, as the Xlib handle `wry` asks for.
pub(super) struct Host(c_ulong);

impl HasWindowHandle for Host {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Xlib(XlibWindowHandle::new(self.0));
        // SAFETY: the app's window outlives the web view.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

pub(super) fn host(window: &Window) -> Result<Host, super::NativeFailure> {
    // GPUI's own `Window::window_handle` shadows the trait's.
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xcb(handle)) => Ok(Host(handle.window.get() as c_ulong)),
        Ok(RawWindowHandle::Xlib(handle)) => Ok(Host(handle.window)),
        _ => Err(super::NativeFailure::new(
            "The in-app browser needs an X11 session; on Wayland the canvas opens in your browser.",
        )),
    }
}

pub(super) struct NativeView {
    webview: wry::WebView,
    /// The profile's WebKit context (its data folder); kept while the view lives.
    _context: Option<wry::WebContext>,
    /// Where it was last placed, and whether it showed.
    placed: Option<(Bounds<Pixels>, bool)>,
}

impl NativeView {
    pub(super) fn new(
        host: Host,
        url: &str,
        background: (u8, u8, u8, u8),
        script: Option<&str>,
        sandbox: Option<&super::Sandbox>,
        profile: &Profile,
        relays: super::Relays,
    ) -> Result<Self, super::NativeFailure> {
        GTK.clone()?;
        // Keep every persistent profile, including Shared, under app data.
        let mut context = (!profile.is_private())
            .then(|| super::profile_dir(profile).map(|dir| wry::WebContext::new(Some(dir))))
            .flatten();
        let builder = match context.as_mut() {
            Some(context) => wry::WebViewBuilder::new_with_web_context(context),
            None => wry::WebViewBuilder::new(),
        }
        .with_incognito(profile.is_private())
        .with_url(url)
        // Runs before the page's own scripts (a device panel names its device).
        .with_initialization_script(script.unwrap_or_default())
        .with_background_color(background)
        .with_visible(false);
        let builder = super::relay(builder, relays);
        let webview = match sandbox {
            Some(sandbox) => super::isolate(builder, sandbox),
            None => builder,
        }
        .build_as_child(&host)
        .map_err(|error| {
            super::NativeFailure::with_error(
                "The browser could not start: {error}. It needs WebKitGTK 4.1.",
                error,
            )
        })?;
        Ok(Self {
            webview,
            _context: context,
            placed: None,
        })
    }

    pub(super) fn place(&mut self, _window: &Window, bounds: Bounds<Pixels>, visible: bool) {
        if self.placed == Some((bounds, visible)) {
            return;
        }
        // GTK only allocates a size to shown widgets, and showing the
        // window wry wraps the page in sizes it to its natural size (about
        // 200 px): the page must be shown when it gets its bounds, or it
        // stays laid out at that size in a corner of the card. A page placed
        // hidden at a new size (parked, or under a screen) is shown for the
        // call and hidden again; it keeps its size.
        let resized = self.placed.is_none_or(|(old, _)| old.size != bounds.size);
        if visible || resized {
            let _ = self.webview.set_visible(true);
        }
        let _ = self.webview.set_bounds(super::logical_rect(bounds));
        if resized {
            use gtk::prelude::*;
            use wry::WebViewExtUnix as _;
            // GTK's own idea of the window's size, for its later layouts.
            if let Some(window) = self
                .webview
                .webview()
                .toplevel()
                .and_then(|w| w.downcast::<gtk::Window>().ok())
            {
                window.resize(
                    bounds.size.width.as_f32().round().max(1.) as i32,
                    bounds.size.height.as_f32().round().max(1.) as i32,
                );
            }
        }
        if !visible {
            let _ = self.webview.set_visible(false);
        }
        self.placed = Some((bounds, visible));
    }

    pub(super) fn load_url(&self, url: &str) {
        let _ = self.webview.load_url(url);
    }

    pub(super) fn reload(&self) {
        let _ = self.webview.reload();
    }

    pub(super) fn evaluate(&self, script: &str) {
        let _ = self.webview.evaluate_script(script);
    }

    /// The page lives in its own native window here, which takes the
    /// keyboard from the app's window, so GPUI never asks while it has it.
    pub(super) fn has_keyboard(&self) -> bool {
        false
    }

    /// Run `script`; `done` gets its value as JSON (nothing if it never ran).
    pub(super) fn evaluate_with(&self, script: &str, done: impl Fn(String) + Send + 'static) {
        let _ = self.webview.evaluate_script_with_callback(script, done);
    }

    /// The page as WebKitGTK draws it, as PNG bytes.
    pub(super) fn screenshot(&self, done: super::Shot) {
        use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt as _};
        use wry::WebViewExtUnix as _;
        self.webview.webview().snapshot(
            SnapshotRegion::Visible,
            SnapshotOptions::NONE,
            None::<&gtk::gio::Cancellable>,
            move |result| {
                done(
                    result
                        .map_err(|error| format!("WebKitGTK couldn't take the picture: {error}"))
                        .and_then(surface_png),
                )
            },
        );
    }

    /// Forget the profile's cookies, storage and caches.
    pub(super) fn clear_data(&self) {
        let _ = self.webview.clear_all_browsing_data();
    }

    /// Hide the view now; the next `place` shows it again.
    pub(super) fn conceal(&mut self) {
        let _ = self.webview.set_visible(false);
        self.placed = None;
    }
}

/// The page lives in its own native window here, which gives the keyboard
/// back when the app's window is clicked.
pub(super) fn reclaim_keyboard(_window: &Window) {}

/// A cairo surface (premultiplied BGRA) as PNG bytes.
fn surface_png(surface: gtk::cairo::Surface) -> Result<Vec<u8>, String> {
    let image = gtk::cairo::ImageSurface::try_from(surface)
        .map_err(|_| "WebKitGTK's picture isn't an image".to_string())?;
    let (width, height, stride) = (
        image.width().max(0) as u32,
        image.height().max(0) as u32,
        image.stride().max(0) as usize,
    );
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    image
        .with_data(|data| {
            for row in data.chunks(stride).take(height as usize) {
                for &[b, g, r, a] in row[..width as usize * 4].as_chunks::<4>().0 {
                    let unmul = |c: u8| {
                        if a == 0 {
                            0
                        } else {
                            (u16::from(c) * 255 / u16::from(a)).min(255) as u8
                        }
                    };
                    rgba.extend_from_slice(&[unmul(r), unmul(g), unmul(b), a]);
                }
            }
        })
        .map_err(|error| error.to_string())?;
    super::png_from_rgba(width, height, rgba)
}
