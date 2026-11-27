//! Canvas tabs, after Tern's canvas blocks: an infinite whiteboard for ink,
//! shapes, arrows, text, sticky notes and images. Ink is perfect-freehand
//! (the algorithm and options Tern's ink stores: size, thinning, smoothing,
//! streamline, easing) drawn natively with GPUI paths; the model, hit
//! testing, camera and undo live in `gentle_core::canvas`.
//!
//! Each canvas is one JSON file under `<data dir>/canvases/`, saved a moment
//! after every change and when the app quits; the pane keeps its path, so
//! the workspace layout restores it. Its Agent button opens an agent beside
//! it that reads, looks at and draws on it (`agent.rs`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gentle_core::canvas::{Doc, History, Kind, P, Rect, Shape, Style};
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::*;

mod agent;
mod chrome;
mod paint;
mod pointer;
#[cfg(test)]
mod tests;

use paint::GeoCache;
use pointer::Gesture;

pub use agent::{AGENT_ENV, install_extension};

pub const CONTEXT: &str = "GentleCanvas";

actions!(
    canvas,
    [
        Undo, Redo, SelectAll, Duplicate, Copy, Cut, Paste, Cancel, Delete
    ]
);

/// What a press on the page does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tool {
    Select,
    Lasso,
    Hand,
    Pen,
    Highlighter,
    Eraser,
    Laser,
    Rect,
    Ellipse,
    Arrow,
    Line,
    Text,
    Note,
    Image,
}

/// What a text field types into.
pub(super) enum EditTarget {
    /// A text, note, rectangle or ellipse on the page.
    Shape(u64),
    /// A new text or note (its place and size), added when typing ends.
    New(Kind),
}

/// A text or note being typed into.
struct TextEdit {
    target: EditTarget,
    input: Entity<TextareaState>,
    _subscription: Subscription,
}

pub struct CanvasView {
    path: PathBuf,
    doc: Doc,
    history: History,
    tool: Tool,
    /// Rectangle or ellipse: what the shape button draws.
    geo_tool: Tool,
    /// Keep the tool after drawing a shape instead of going back to Select.
    tool_lock: bool,
    /// The style new shapes take; the selection's when one is picked.
    style: Style,
    /// The style panel edits the fill color.
    fill_tab: bool,
    selected: Vec<u64>,
    gesture: Option<Gesture>,
    /// Laser trail points (page) and when each was drawn.
    laser: Vec<(P, Instant)>,
    /// The pointer over the page (canvas px), for the eraser's ring.
    hover: Option<P>,
    /// Space is held: dragging pans.
    space_pan: bool,
    editing: Option<TextEdit>,
    minimap: bool,
    /// The page element's bounds last frame.
    bounds: Rc<Cell<Bounds<Pixels>>>,
    minimap_bounds: Rc<Cell<Bounds<Pixels>>>,
    geo: GeoCache,
    focus: FocusHandle,
    /// The file could not be read: never overwrite it.
    broken: Option<crate::shared::i18n::Message>,
    save_task: Option<Task<()>>,
    _quit: Subscription,
    /// Answers the agent's requests.
    _requests: Task<()>,
    /// The folder the agent works in (the pane's).
    cwd: PathBuf,
    /// The agent floating over the canvas, once opened.
    agent: Option<agent::FloatingAgent>,
    /// The conversation the agent had before a restart, resumed when it opens.
    agent_resume: Option<PathBuf>,
}

/// What the canvas tells the window.
pub enum CanvasEvent {
    /// Its agent started: the window handles what it asks for (pages,
    /// notices, screens) as it does for an agent pane's.
    AgentStarted(Entity<crate::features::session::SessionView>),
}

impl EventEmitter<CanvasEvent> for CanvasView {}

impl Focusable for CanvasView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Where new canvases are saved; `None` without a data directory.
pub fn canvases_dir() -> Option<PathBuf> {
    crate::shared::data_dir::app().map(|d| d.join("canvases"))
}

/// A path for a new canvas file.
pub fn new_canvas_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    canvases_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("canvas-{stamp:x}.json"))
}

pub fn bind_keys(cx: &mut App) {
    macro_rules! key {
        ($k:literal) => {
            if cfg!(target_os = "macos") {
                concat!("cmd-", $k)
            } else {
                concat!("ctrl-", $k)
            }
        };
    }
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new(key!("z"), Undo, ctx),
        KeyBinding::new(key!("shift-z"), Redo, ctx),
        KeyBinding::new(key!("y"), Redo, ctx),
        KeyBinding::new(key!("a"), SelectAll, ctx),
        KeyBinding::new(key!("d"), Duplicate, ctx),
        KeyBinding::new(key!("c"), Copy, ctx),
        KeyBinding::new(key!("x"), Cut, ctx),
        KeyBinding::new(key!("v"), Paste, ctx),
        KeyBinding::new("escape", Cancel, ctx),
        KeyBinding::new("backspace", Delete, ctx),
        KeyBinding::new("delete", Delete, ctx),
    ]);
}

impl CanvasView {
    pub fn open(path: PathBuf, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (doc, broken) = match std::fs::read_to_string(&path) {
            Ok(json) => match Doc::parse(&json) {
                Ok(doc) => (doc, None),
                Err(e) => (
                    Doc::default(),
                    Some(crate::shared::i18n::Message::with_error(
                        "Couldn't read this canvas: {error}",
                        e,
                    )),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Doc::default(), None),
            Err(e) => (
                Doc::default(),
                Some(crate::shared::i18n::Message::with_error(
                    "Couldn't open this canvas: {error}",
                    e,
                )),
            ),
        };
        // A change made in the last half second is still waiting to be saved.
        let quit = cx.on_app_quit(|this, cx| {
            if let Some(job) = this.save_job() {
                job();
            }
            if let Some(agent) = &this.agent {
                agent.session.read(cx).shutdown_backend();
            }
            async {}
        });
        let requests = Self::watch_agent(path.clone(), cx);
        Self {
            path,
            doc,
            history: History::default(),
            tool: Tool::Pen,
            geo_tool: Tool::Rect,
            tool_lock: false,
            style: Style::default(),
            fill_tab: false,
            selected: Vec::new(),
            gesture: None,
            laser: Vec::new(),
            hover: None,
            space_pan: false,
            editing: None,
            minimap: false,
            bounds: Rc::new(Cell::new(Bounds::default())),
            minimap_bounds: Rc::new(Cell::new(Bounds::default())),
            geo: Rc::new(RefCell::new(HashMap::new())),
            focus: cx.focus_handle(),
            broken,
            save_task: None,
            _quit: quit,
            _requests: requests,
            cwd: dirs::home_dir().unwrap_or_default(),
            agent: None,
            agent_resume: None,
        }
    }

    pub(crate) const TITLE: &str = "Canvas";

    pub fn title(&self) -> SharedString {
        crate::shared::i18n::t(Self::TITLE).into()
    }

    // ── Changes and saving ─────────────────────────────────────────────────

    /// Call before changing shapes: one undo step.
    fn record(&mut self) {
        self.history.record(self.doc.shapes.clone());
    }

    /// Shapes `ids` changed: drop their tessellation and save soon.
    fn touched(&mut self, ids: &[u64], cx: &mut Context<Self>) {
        let mut geo = self.geo.borrow_mut();
        for id in ids {
            geo.remove(id);
        }
        drop(geo);
        self.schedule_save(cx);
        cx.notify();
    }

    /// Every shape may have changed (undo, redo).
    fn touched_all(&mut self, cx: &mut Context<Self>) {
        self.geo.borrow_mut().clear();
        let live: Vec<u64> = self.doc.shapes.iter().map(|s| s.id).collect();
        self.selected.retain(|id| live.contains(id));
        self.schedule_save(cx);
        cx.notify();
    }

    /// Adds `shape` on top (one undo step) and returns its id.
    fn add(&mut self, mut shape: Shape, cx: &mut Context<Self>) -> u64 {
        self.record();
        shape.id = self.doc.next_id();
        let id = shape.id;
        self.doc.shapes.push(shape);
        self.touched(&[id], cx);
        id
    }

    /// The file write for the current page; `None` when it must not be written.
    fn save_job(&self) -> Option<impl FnOnce() + Send + 'static + use<>> {
        if self.broken.is_some() {
            return None;
        }
        let json = self.doc.to_json();
        let path = self.path.clone();
        Some(move || {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        })
    }

    /// Saves half a second after the last change.
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let Ok(Some(job)) = this.update(cx, |this, _| this.save_job().map(Box::new)) else {
                return;
            };
            cx.background_executor().spawn(async move { job() }).await;
        }));
    }

    // ── Style ──────────────────────────────────────────────────────────────

    /// Applies `change` to the style new shapes take and to the selection;
    /// each selected shape takes only what shows on it.
    fn restyle(&mut self, change: impl Fn(&mut Style), cx: &mut Context<Self>) {
        change(&mut self.style);
        if !self.selected.is_empty() {
            self.record();
            let ids = self.selected.clone();
            for s in self.doc.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                let mut next = s.style;
                change(&mut next);
                s.style.adopt(&next, s.kind.controls());
            }
            self.touched(&ids, cx);
        }
        cx.notify();
    }

    fn select(&mut self, ids: Vec<u64>, cx: &mut Context<Self>) {
        if let Some(first) = ids.first().and_then(|id| self.doc.get(*id)) {
            self.style.adopt(&first.style, first.kind.controls());
        }
        self.selected = ids;
        cx.notify();
    }

    fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_edit(window, cx);
        if matches!(tool, Tool::Rect | Tool::Ellipse) {
            self.geo_tool = tool;
        }
        if !matches!(tool, Tool::Select | Tool::Lasso) {
            self.selected.clear();
        }
        if tool == Tool::Image {
            self.pick_images(None, window, cx);
            return;
        }
        self.tool = tool;
        cx.notify();
    }

    /// After drawing a shape: back to Select unless the tool is locked.
    fn after_create(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.tool_lock {
            self.tool = Tool::Select;
            self.select(vec![id], cx);
        }
    }

    // ── Camera ─────────────────────────────────────────────────────────────

    fn size(&self) -> (f32, f32) {
        let s = self.bounds.get().size;
        (f32::from(s.width), f32::from(s.height))
    }

    fn zoom_center(&mut self, factor: f32, cx: &mut Context<Self>) {
        let (w, h) = self.size();
        self.doc.camera.zoom_at(factor, [w / 2., h / 2.]);
        self.camera_moved(cx);
    }

    fn zoom_to(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let factor = zoom / self.doc.camera.zoom;
        self.zoom_center(factor, cx);
    }

    fn fit(&mut self, cx: &mut Context<Self>) {
        let (w, h) = self.size();
        let content = if self.selected.is_empty() {
            self.doc.content_bounds()
        } else {
            gentle_core::canvas::bounds_of(
                self.doc
                    .shapes
                    .iter()
                    .filter(|s| self.selected.contains(&s.id)),
            )
        };
        match content {
            Some(content) => {
                self.doc.camera = gentle_core::canvas::Camera::fitting(&content, w, h, 64.)
            }
            None => {
                self.doc.camera = gentle_core::canvas::Camera {
                    x: -w / 2.,
                    y: -h / 2.,
                    zoom: 1.,
                }
            }
        }
        self.camera_moved(cx);
    }

    fn camera_moved(&mut self, cx: &mut Context<Self>) {
        self.schedule_save(cx);
        cx.notify();
    }

    /// The page point at the middle of the view.
    fn view_center(&self) -> P {
        let (w, h) = self.size();
        self.doc.camera.to_page([w / 2., h / 2.])
    }

    // ── Text ───────────────────────────────────────────────────────────────

    /// Opens the text field on `target`.
    fn start_edit(&mut self, target: EditTarget, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_edit(window, cx);
        let id = match target {
            EditTarget::Shape(id) => Some(id),
            EditTarget::New(_) => None,
        };
        let text = match id.and_then(|id| self.doc.get(id)).map(|s| &s.kind) {
            Some(
                Kind::Text { text, .. }
                | Kind::Note { text, .. }
                | Kind::Rect { label: text, .. }
                | Kind::Ellipse { label: text, .. },
            ) => text.clone(),
            _ => String::new(),
        };
        let input = cx.new(|cx| TextareaState::new(window, cx).default_value(text));
        input.update(cx, |input, cx| input.focus(window, cx));
        // Opened from a press on the page: keep the page's own
        // focus-on-click (it runs after this) from taking focus back.
        window.prevent_default();
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if let InputEvent::Blur = event {
                this.finish_edit(window, cx);
            }
        });
        self.selected = id.into_iter().collect();
        self.editing = Some(TextEdit {
            target,
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Ends typing: saves the text into its shape. Empty text removes a
    /// text shape (a note stays).
    fn finish_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        window.focus(&self.focus, cx);
        let value = edit.input.read(cx).value().trim_end().to_string();
        cx.notify();
        match edit.target {
            EditTarget::Shape(id) => {
                let Some(old) = self.doc.get(id).map(|s| &s.kind) else {
                    return;
                };
                let unchanged = matches!(old,
                    Kind::Text { text, .. }
                    | Kind::Note { text, .. }
                    | Kind::Rect { label: text, .. }
                    | Kind::Ellipse { label: text, .. } if *text == value);
                let gone = value.is_empty() && matches!(old, Kind::Text { .. });
                if unchanged {
                    return;
                }
                self.record();
                if gone {
                    self.doc.remove(&[id]);
                    self.selected.clear();
                } else if let Some(shape) = self.doc.get_mut(id)
                    && let Kind::Text { text, .. }
                    | Kind::Note { text, .. }
                    | Kind::Rect { label: text, .. }
                    | Kind::Ellipse { label: text, .. } = &mut shape.kind
                {
                    *text = value;
                }
                self.touched(&[id], cx);
            }
            EditTarget::New(mut kind) => {
                match &mut kind {
                    Kind::Text { text, .. } if !value.is_empty() => *text = value,
                    Kind::Note { text, .. } => *text = value,
                    _ => return,
                }
                let id = self.add(
                    Shape {
                        id: 0,
                        kind,
                        style: self.style,
                    },
                    cx,
                );
                self.after_create(id, cx);
            }
        }
    }

    // ── Images ─────────────────────────────────────────────────────────────

    /// Asks for image files and places them at `at` (page), else the middle.
    fn pick_images(&mut self, at: Option<P>, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(crate::shared::i18n::t("Insert").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.insert_images(paths, at, cx));
        })
        .detach();
    }

    /// Image files become image shapes, side by side from `at`.
    fn insert_images(&mut self, paths: Vec<PathBuf>, at: Option<P>, cx: &mut Context<Self>) {
        let mut at = at.unwrap_or_else(|| self.view_center());
        let mut ids = Vec::new();
        for path in paths.into_iter().filter(|p| is_image(p)) {
            let Ok(dim) = imagesize::size(&path) else {
                continue;
            };
            let (w, h) = (dim.width as f32, dim.height as f32);
            let k = (480. / w.max(h)).min(1.);
            let (w, h) = (w * k, h * k);
            self.record();
            let id = self.doc.next_id();
            self.doc.shapes.push(Shape {
                id,
                kind: Kind::Image {
                    rect: Rect {
                        x: at[0] - w / 2.,
                        y: at[1] - h / 2.,
                        w,
                        h,
                    },
                    path: path.to_string_lossy().into_owned(),
                },
                style: Style::default(),
            });
            ids.push(id);
            at[0] += w + 24.;
        }
        if !ids.is_empty() {
            self.touched(&ids, cx);
            self.tool = Tool::Select;
            self.select(ids, cx);
        }
    }

    /// Image bytes from the clipboard: kept beside the canvases, then placed.
    fn insert_image_bytes(&mut self, image: &Image, cx: &mut Context<Self>) {
        use sha2::Digest as _;
        let Some(dir) = canvases_dir().map(|d| d.join("assets")) else {
            return;
        };
        let ext = match image.format {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
            ImageFormat::Gif => "gif",
            ImageFormat::Svg => "svg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tiff",
            _ => return,
        };
        let hash = sha2::Sha256::digest(&image.bytes);
        let name: String = hash.iter().take(12).map(|b| format!("{b:02x}")).collect();
        let path = dir.join(format!("{name}.{ext}"));
        if !path.exists()
            && (std::fs::create_dir_all(&dir).is_err()
                || std::fs::write(&path, &image.bytes).is_err())
        {
            return;
        }
        self.insert_images(vec![path], None, cx);
    }
}

fn is_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| {
            matches!(
                e.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "svg"
            )
        })
}

impl Render for CanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::shared::diagnostics::render("CanvasView");
        // The agent floats beside the page, outside its keys and pointer.
        div()
            .relative()
            .size_full()
            .child(self.render_page(window, cx))
            .children(self.agent_panel(cx))
    }
}
