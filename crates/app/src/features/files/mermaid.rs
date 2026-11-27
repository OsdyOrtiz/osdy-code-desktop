//! Mermaid diagrams in Markdown: the file preview and the conversation.
//! Each ` ```mermaid ` block is drawn as an SVG image by merman's CLI, the
//! *diagram renderer*: not part of the app, it is downloaded the first time
//! someone asks for it (see [`install`]). A block shows as its source until
//! it is drawn (off the UI thread), and stays source with the error under it
//! when it can't be.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write as _;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use gentle_core::markdown::mermaid_blocks;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _, h_flex};
use gpui_kit::{
    App, BorrowAppContext as _, Div, ElementId, Global, Image, ImageFormat, ImageSource,
    IntoElement as _, ParentElement as _, SharedUri, Styled as _, Task, div,
};

use super::Text;
use crate::shared::{i18n::t, ui};

const START_DOWNLOAD: &str = "Starting the download…";

/// How a document's images name a drawn diagram: `mermaid:<key>`.
const SCHEME: &str = "mermaid:";
/// Drawn diagrams kept per preview; a long typing session makes many.
const KEEP: usize = 64;

/// The diagram renderer, app-wide: previews and the Docs screen follow it.
#[derive(Clone, Debug, PartialEq)]
pub enum Renderer {
    Missing,
    Installing(String),
    Ready(PathBuf),
    Failed(String),
}

pub struct RendererState {
    renderer: Renderer,
    _install: Task<()>,
}

impl Global for RendererState {}

pub(super) fn init(cx: &mut App) {
    let renderer =
        crate::features::setup::diagram_renderer().map_or(Renderer::Missing, Renderer::Ready);
    cx.set_global(RendererState {
        renderer,
        _install: Task::ready(()),
    });
}

/// The diagram renderer's state (`Missing` before [`init`], as in tests
/// that don't set the app up).
pub fn renderer(cx: &App) -> &Renderer {
    static MISSING: Renderer = Renderer::Missing;
    cx.try_global::<RendererState>()
        .map_or(&MISSING, |state| &state.renderer)
}

/// The diagram renderer, once installed.
pub fn ready_cli(cx: &App) -> Option<PathBuf> {
    match renderer(cx) {
        Renderer::Ready(cli) => Some(cli.clone()),
        _ => None,
    }
}

/// The app's theme is dark: Mermaid draws in its dark theme.
pub fn dark(cx: &App) -> bool {
    cx.try_global::<crate::shared::themes::Themes>()
        .is_none_or(|themes| !themes.active_theme().light)
}

/// Where `blocks` Mermaid blocks show and no renderer draws them: what they
/// are and a way to draw them (a one-time download, not part of the app);
/// `None` once the renderer is there. `id` names the button.
pub fn banner(blocks: usize, id: impl Into<ElementId>, cx: &App) -> Option<Div> {
    let (line, action) = match renderer(cx) {
        _ if blocks == 0 => return None,
        Renderer::Ready(_) => return None,
        Renderer::Missing => (
            t(if blocks == 1 {
                "{blocks} Mermaid diagram here. Draw it with the diagram renderer, a one-time download of about 20 MB."
            } else {
                "{blocks} Mermaid diagrams here. Draw them with the diagram renderer, a one-time download of about 20 MB."
            }).replace("{blocks}", &blocks.to_string()),
            Some(t("Install")),
        ),
        Renderer::Installing(progress) => ((if progress == START_DOWNLOAD { t(START_DOWNLOAD) } else { progress }).to_string(), None),
        Renderer::Failed(error) => (
            t("The diagram renderer couldn't be installed: {error}").replace("{error}", error),
            Some(t("Try again")),
        ),
    };
    let installing = matches!(renderer(cx), Renderer::Installing(_));
    Some(
        h_flex()
            .px_3()
            .py_2()
            .gap_2()
            .rounded_lg()
            .bg(ui::surface())
            .text_xs()
            .text_color(ui::muted())
            .child(if installing {
                Spinner::new()
                    .xsmall()
                    .color(ui::accent())
                    .into_any_element()
            } else {
                Icon::new(IconName::Workflow)
                    .xsmall()
                    .text_color(ui::info())
                    .into_any_element()
            })
            .child(div().flex_1().min_w_0().child(line))
            .children(action.map(|label| {
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .label(label)
                    .on_click(|_, _, cx| install(cx))
            })),
    )
}

/// Download and install the diagram renderer; observers of the global (the
/// previews, the Docs screen) redraw as it goes.
pub fn install(cx: &mut App) {
    if matches!(renderer(cx), Renderer::Installing(_) | Renderer::Ready(_)) {
        return;
    }
    let set = |renderer: Renderer, cx: &mut App| {
        cx.update_global::<RendererState, _>(|state, _| state.renderer = renderer);
    };
    set(Renderer::Installing(START_DOWNLOAD.into()), cx);
    let (tx, rx) = async_channel::unbounded::<String>();
    let task = cx.spawn(async move |cx| {
        let work = cx.background_executor().spawn(async move {
            crate::features::setup::install_diagram_renderer(&move |line| {
                let _ = tx.try_send(line);
            })
        });
        while let Ok(line) = rx.recv().await {
            let line = line.trim().to_string();
            cx.update(|cx| set(Renderer::Installing(line), cx));
        }
        let done = work.await;
        cx.update(|cx| {
            set(
                match done {
                    Ok(path) => Renderer::Ready(path),
                    Err(error) => Renderer::Failed(error),
                },
                cx,
            )
        });
    });
    cx.global_mut::<RendererState>()._install = task;
}

#[derive(Clone)]
enum Drawing {
    Pending,
    Ready(Arc<Image>),
    Failed(Text),
}

/// The diagrams of one preview, by source and theme.
#[derive(Default)]
pub struct Diagrams(Mutex<HashMap<u64, Drawing>>);

/// A diagram to draw: its key, its source, the theme and the renderer.
pub struct Job {
    key: u64,
    source: String,
    dark: bool,
    cli: PathBuf,
}

/// What [`Diagrams::prepare`] gives the preview.
pub struct Prepared {
    /// The document as shown.
    pub text: String,
    /// Mermaid blocks in it.
    pub blocks: usize,
    /// Diagrams still to draw (only with a renderer).
    pub jobs: Vec<Job>,
}

impl Diagrams {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Drawing>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `text` as the preview shows it: drawn diagrams as images, the others
    /// as their source (with the error under a failed one). Without a
    /// renderer (`cli`) nothing is drawn.
    pub fn prepare(&self, text: &str, dark: bool, cli: Option<&PathBuf>) -> Prepared {
        let blocks = mermaid_blocks(text);
        let mut prepared = Prepared {
            text: String::with_capacity(text.len()),
            blocks: blocks.len(),
            jobs: Vec::new(),
        };
        let Some(cli) = cli.filter(|_| !blocks.is_empty()) else {
            prepared.text.push_str(text);
            return prepared;
        };
        let mut cache = self.lock();
        let mut at = 0;
        for block in blocks {
            let source = &text[block.source.clone()];
            let key = key(source, dark);
            let out = &mut prepared.text;
            out.push_str(&text[at..block.range.start]);
            match cache.get(&key) {
                Some(Drawing::Ready(_)) => {
                    out.push_str(&format!("![{}]({SCHEME}{key})\n", t("Mermaid diagram")));
                }
                Some(Drawing::Failed(error)) => {
                    out.push_str(&text[block.range.clone()]);
                    out.push_str("\n> ");
                    out.push_str(
                        &t("Mermaid couldn't draw this diagram: {error}")
                            .replace("{error}", &error.render()),
                    );
                    out.push('\n');
                }
                Some(Drawing::Pending) => out.push_str(&text[block.range.clone()]),
                None => {
                    out.push_str(&text[block.range.clone()]);
                    cache.insert(key, Drawing::Pending);
                    prepared.jobs.push(Job {
                        key,
                        source: source.to_string(),
                        dark,
                        cli: cli.clone(),
                    });
                }
            }
            at = block.range.end;
        }
        prepared.text.push_str(&text[at..]);
        if cache.len() > KEEP {
            // Edits leave old versions behind; keep only what is pending.
            cache.retain(|_, d| matches!(d, Drawing::Pending));
        }
        prepared
    }

    /// Store what drawing `job` gave.
    pub fn finish(&self, job: &Job, drawn: Result<Vec<u8>, Text>) {
        let drawing = match drawn {
            Ok(svg) => Drawing::Ready(Arc::new(Image::from_bytes(ImageFormat::Svg, svg))),
            Err(error) => Drawing::Failed(error),
        };
        self.lock().insert(job.key, drawing);
    }

    /// The image for a `mermaid:<key>` URI, if it is one.
    pub fn image(&self, uri: &SharedUri) -> Option<ImageSource> {
        let key: u64 = uri.strip_prefix(SCHEME)?.parse().ok()?;
        match self.lock().get(&key)? {
            Drawing::Ready(image) => Some(ImageSource::Image(image.clone())),
            _ => None,
        }
    }
}

fn key(source: &str, dark: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    dark.hash(&mut hasher);
    hasher.finish()
}

/// The sans font the renderer measures text with: the one GPUI's SVG
/// renderer will draw it with, so labels fit their boxes. Found once
/// (loading the font database takes a moment).
static FONT: LazyLock<Option<(String, PathBuf)>> = LazyLock::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let families = [
        "Arial",
        "Helvetica",
        "Segoe UI",
        "DejaVu Sans",
        "Liberation Sans",
    ]
    .map(fontdb::Family::Name);
    let face = families
        .iter()
        .chain([&fontdb::Family::SansSerif])
        .find_map(|family| {
            let id = db.query(&fontdb::Query {
                families: std::slice::from_ref(family),
                ..fontdb::Query::default()
            })?;
            let face = db.face(id)?;
            let path = match &face.source {
                fontdb::Source::File(path) => path.clone(),
                fontdb::Source::SharedFile(path, _) => path.clone(),
                fontdb::Source::Binary(_) => return None,
            };
            Some((face.families.first()?.0.clone(), path))
        })?;
    Some(face)
});

/// Draw `job` as SVG with the renderer. Tens of milliseconds: call it off
/// the UI thread.
pub fn draw(job: &Job) -> Result<Vec<u8>, Text> {
    let mut init = vec![format!(
        "\"theme\": \"{}\"",
        if job.dark { "dark" } else { "default" }
    )];
    let mut command = pi_rpc::command(&job.cli);
    command.args([
        "render",
        "-",
        "-o",
        "-",
        "-f",
        "svg",
        "--svg-pipeline",
        "resvg-safe",
        "-q",
    ]);
    if let Some((family, path)) = FONT.as_ref() {
        command.arg("--font-file").arg(path);
        init.push(format!("\"fontFamily\": \"{family}\""));
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            Text::from("couldn't start the diagram renderer: {error}")
                .with("{error}", e.to_string())
        })?;
    let source = format!("%%{{init: {{{}}}}}%%\n{}", init.join(", "), job.source);
    child
        .stdin
        .take()
        .ok_or("the diagram renderer has no input")?
        .write_all(source.as_bytes())
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() && !output.stdout.is_empty() {
        Ok(output.stdout)
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if error.is_empty() {
            "the diagram renderer gave no drawing".into()
        } else {
            error.into()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Diagrams, draw};

    const DOC: &str = "# Flow\n\n```mermaid\nflowchart LR\n  a[Start] --> b[End]\n```\n\nafter\n";

    #[test]
    fn without_a_renderer_diagrams_stay_source_and_nothing_is_drawn() {
        let prepared = Diagrams::default().prepare(DOC, true, None);
        assert_eq!(prepared.text, DOC);
        assert_eq!(prepared.blocks, 1);
        assert!(prepared.jobs.is_empty());
    }

    #[test]
    fn a_diagram_shows_as_source_until_drawn_then_as_an_image() {
        let diagrams = Diagrams::default();
        let cli = PathBuf::from("merman-cli");
        let prepared = diagrams.prepare(DOC, true, Some(&cli));
        assert_eq!(prepared.text, DOC);
        assert_eq!(prepared.jobs.len(), 1);
        // Asked again while drawing: still the source, no second job.
        assert!(diagrams.prepare(DOC, true, Some(&cli)).jobs.is_empty());

        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'/>".to_vec();
        diagrams.finish(&prepared.jobs[0], Ok(svg));
        let shown = diagrams.prepare(DOC, true, Some(&cli)).text;
        assert!(shown.starts_with("# Flow\n\n![Mermaid diagram](mermaid:"));
        assert!(shown.ends_with(")\n\nafter\n"));
        let uri = shown.split('(').nth(1).unwrap().split(')').next().unwrap();
        assert!(diagrams.image(&uri.to_string().into()).is_some());
        // The other theme is another drawing.
        assert_eq!(diagrams.prepare(DOC, false, Some(&cli)).jobs.len(), 1);
    }

    #[test]
    fn a_diagram_that_cannot_be_drawn_keeps_its_source_and_says_why() {
        let doc = "```mermaid\nnot a diagram at all\n```\n";
        let diagrams = Diagrams::default();
        let cli = PathBuf::from("/nonexistent/merman-cli");
        let prepared = diagrams.prepare(doc, false, Some(&cli));
        let drawn = draw(&prepared.jobs[0]);
        assert!(drawn.is_err());
        diagrams.finish(&prepared.jobs[0], drawn);
        let shown = diagrams.prepare(doc, false, Some(&cli)).text;
        assert!(shown.starts_with(doc));
        assert!(shown.contains("> Mermaid couldn't draw this diagram:"));
    }
}
