//! Resource diagnostics (docs/development.md, Diagnostics). Started from
//! Configuration › General, or from launch with
//! `GENTLE_SHELL_GPUI_DIAGNOSTICS=1` or `--diagnostics`, a recording appends
//! one line per second to `<data dir>/diagnostics/diagnostics-<UTC>.jsonl`:
//! the app's own CPU and memory, the counters of
//! [`crate::shared::diagnostics`] (frames by source line, renders, poll
//! wakeups), and every tab's backend with all the processes under it
//! (subagents, jobs, tools). Nothing leaves the machine;
//! `--diagnostics-report [FILE]` ranks a recording.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gentle_core::diagnostics::{self as model, AppSample, Header, HeaderLine, Sample, TabSample};
use gpui_kit::{App, Global, Task, WeakEntity};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::features::session::SessionView;
use crate::shared::diagnostics::{enabled, set_enabled};

/// Prints the ranking of a recording (the newest without a path) and exits.
pub const REPORT_FLAG: &str = "--diagnostics-report";
const INTERVAL: Duration = Duration::from_secs(1);
/// A recording stops growing here (about nine hours at one line a second).
const MAX_BYTES: u64 = 64 << 20;
/// Recordings kept in the folder, this one included.
const KEEP: usize = 10;

/// What a tab reports of itself each sample (`SessionView::resources`).
pub struct TabResources {
    /// The backend's pid and the records and bytes it has sent.
    pub backend: Option<(u32, u64, u64)>,
    pub agents: usize,
    pub agents_running: usize,
    pub jobs: usize,
    pub jobs_running: usize,
    pub items: usize,
}

thread_local! {
    /// Every open tab, numbered in the order they opened.
    static TABS: RefCell<Vec<(u64, WeakEntity<SessionView>)>> = const { RefCell::new(Vec::new()) };
    static NEXT_TAB: Cell<u64> = const { Cell::new(1) };
}

/// Follow a new tab, so a recording started later sees it.
pub fn watch_tab(tab: WeakEntity<SessionView>) {
    let n = NEXT_TAB.replace(NEXT_TAB.get() + 1);
    TABS.with_borrow_mut(|tabs| {
        tabs.retain(|(_, tab)| tab.upgrade().is_some());
        tabs.push((n, tab));
    });
}

/// The recording in progress.
struct Recording {
    path: PathBuf,
    _sampler: Task<()>,
}

struct Recordings(Option<Recording>);

impl Global for Recordings {}

/// The folder recordings go in.
pub fn folder() -> Option<PathBuf> {
    crate::shared::data_dir::app().map(|dir| dir.join("diagnostics"))
}

/// The newest recording in [`folder`].
pub fn latest() -> Option<PathBuf> {
    let dir = folder()?;
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("diagnostics-") && name.ends_with(".jsonl"))
        .collect();
    names.sort();
    Some(dir.join(names.pop()?))
}

/// The running recording's file.
pub fn recording(cx: &App) -> Option<&Path> {
    let recording = cx.try_global::<Recordings>()?.0.as_ref()?;
    enabled().then_some(recording.path.as_path())
}

/// Record from launch when this launch asked to. Called once, at launch.
pub fn start(cx: &mut App) {
    if crate::shared::diagnostics::requested_at_launch()
        && let Err(error) = start_recording(cx)
    {
        eprintln!("Diagnostics: {error}");
    }
}

/// Start a new recording (ending any running one); its file's path.
pub fn start_recording(cx: &mut App) -> Result<PathBuf, String> {
    stop_recording(cx);
    use crate::shared::i18n::t;

    let dir = folder().ok_or_else(|| t("no data folder to record into"))?;
    let recorder = Recorder::open(&dir).map_err(|error| {
        gentle_core::i18n::fill(
            t("could not start a recording in {folder}: {error}"),
            &[
                ("folder", &dir.to_string_lossy()),
                ("error", &error.to_string()),
            ],
        )
    })?;
    let path = recorder.path.clone();
    eprintln!("Diagnostics: recording to {}", path.display());
    set_enabled(true);
    let sampler = cx.spawn(async move |cx| {
        let mut recorder = recorder;
        let mut last = Instant::now();
        // Each tab's backend pid and its traffic counters last sample.
        let mut seen: HashMap<u64, (u32, u64, u64)> = HashMap::new();
        loop {
            cx.background_executor().timer(INTERVAL).await;
            let now = Instant::now();
            let seconds = now.duration_since(last).as_secs_f64();
            last = now;
            let (app, tabs) = cx.update(|cx| snapshot(seconds, cx));
            let tabs = tabs
                .into_iter()
                .map(|(tab, resources)| rate_tab(tab, resources, seconds, &mut seen))
                .collect();
            let (back, ok) = cx
                .background_executor()
                .spawn(async move {
                    let ok = recorder.record(app, tabs);
                    (recorder, ok)
                })
                .await;
            if !ok {
                break;
            }
            recorder = back;
        }
        // Full or unwritable: the recording ends on its own.
        set_enabled(false);
        cx.update(|cx| cx.refresh_windows());
    });
    cx.set_global(Recordings(Some(Recording {
        path: path.clone(),
        _sampler: sampler,
    })));
    Ok(path)
}

/// End the running recording, if any; its file stays.
pub fn stop_recording(cx: &mut App) {
    set_enabled(false);
    // Dropping the sampler ends it.
    let ended = cx
        .try_global::<Recordings>()
        .and_then(|recordings| Some(recordings.0.as_ref()?.path.clone()));
    if let Some(path) = ended {
        cx.set_global(Recordings(None));
        eprintln!("Diagnostics: recording saved to {}", path.display());
    }
}

fn snapshot(seconds: f64, cx: &mut App) -> (AppSample, Vec<(u64, TabResources)>) {
    let [frames, renders, ticks] =
        crate::shared::diagnostics::take().map(|counts| model::rates(&counts, seconds));
    let theme = cx
        .try_global::<crate::shared::themes::Themes>()
        .map(|themes| themes.active_theme().name.clone())
        .unwrap_or_default();
    let app = AppSample {
        windows: cx.windows().len(),
        theme,
        motion: crate::shared::backdrop::motion_now(cx).label().to_string(),
        frames,
        renders,
        ticks,
        ..Default::default()
    };
    let tabs = TABS.with_borrow_mut(|tabs| {
        tabs.retain(|(_, tab)| tab.upgrade().is_some());
        tabs.iter()
            .filter_map(|(n, tab)| Some((*n, tab.upgrade()?.read(cx).resources())))
            .collect()
    });
    (app, tabs)
}

fn rate_tab(
    tab: u64,
    resources: TabResources,
    seconds: f64,
    seen: &mut HashMap<u64, (u32, u64, u64)>,
) -> (TabSample, Option<u32>) {
    let mut sample = TabSample {
        tab,
        agents: resources.agents,
        agents_running: resources.agents_running,
        jobs: resources.jobs,
        jobs_running: resources.jobs_running,
        items: resources.items,
        ..Default::default()
    };
    let pid = resources.backend.map(|(pid, records, bytes)| {
        // A restarted backend counts from zero again.
        let (records_before, bytes_before) = match seen.insert(tab, (pid, records, bytes)) {
            Some((same, r, b)) if same == pid => (r, b),
            _ => (0, 0),
        };
        let per_second = |n: u64| ((n as f64 / seconds.max(f64::EPSILON)) * 10.).round() / 10.;
        sample.rpc_records = per_second(records.saturating_sub(records_before));
        sample.rpc_kb = per_second(bytes.saturating_sub(bytes_before) / 1024);
        pid
    });
    (sample, pid)
}

struct Recorder {
    system: System,
    file: std::fs::File,
    path: PathBuf,
    written: u64,
    pid: Pid,
}

impl Recorder {
    /// A new recording in `dir`, its header written; the oldest recordings
    /// past [`KEEP`] are deleted.
    fn open(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let names: Vec<String> = std::fs::read_dir(dir)?
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .collect();
        for name in model::stale(&names, KEEP - 1) {
            let _ = std::fs::remove_file(dir.join(name));
        }
        let path = dir.join(model::file_name(now_ms()));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let pid = sysinfo::get_current_pid().map_err(std::io::Error::other)?;
        let mut system = System::new();
        system.refresh_memory();
        let mut recorder = Self {
            system,
            file,
            path,
            written: 0,
            pid,
        };
        // CPU use is measured between two refreshes: this is the first.
        recorder.refresh();
        let header = HeaderLine {
            header: Header {
                version: env!("CARGO_PKG_VERSION").to_string(),
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
                cores: std::thread::available_parallelism().map_or(0, |n| n.get()),
                memory_mb: model::mb(recorder.system.total_memory()),
                interval_ms: INTERVAL.as_millis() as u64,
            },
        };
        recorder.write(&serde_json::to_string(&header)?)?;
        Ok(recorder)
    }

    fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );
    }

    /// Measure the processes and append one sample; false once the
    /// recording is full or can't be written.
    fn record(&mut self, mut app: AppSample, tabs: Vec<(TabSample, Option<u32>)>) -> bool {
        self.refresh();
        let table: Vec<model::Process> = self
            .system
            .processes()
            .values()
            // Linux lists threads as tasks; they are counted in their process.
            .filter(|p| p.thread_kind().is_none())
            .map(|p| model::Process {
                pid: p.pid().as_u32(),
                parent: p.parent().map(Pid::as_u32),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                rss: p.memory(),
            })
            .collect();
        if let Some(own) = self.system.process(self.pid) {
            app.cpu = (own.cpu_usage() * 10.).round() / 10.;
            app.rss_mb = model::mb(own.memory());
        }
        let mut trees = model::trees(&table, self.pid.as_u32());
        let tabs = tabs
            .into_iter()
            .map(|(mut tab, pid)| {
                tab.tree = pid.and_then(|pid| trees.remove(&pid));
                tab
            })
            .collect();
        let sample = Sample {
            t: now_ms(),
            app,
            tabs,
            others: trees.into_values().collect(),
        };
        let line = match serde_json::to_string(&sample) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("Diagnostics: could not encode a sample: {error}");
                return false;
            }
        };
        if self.written + line.len() as u64 > MAX_BYTES {
            eprintln!(
                "Diagnostics: {} reached {} MB; recording stopped",
                self.path.display(),
                MAX_BYTES >> 20
            );
            return false;
        }
        match self.write(&line) {
            Ok(()) => true,
            Err(error) => {
                eprintln!(
                    "Diagnostics: could not write {}: {error}",
                    self.path.display()
                );
                false
            }
        }
    }

    fn write(&mut self, line: &str) -> std::io::Result<()> {
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.written += line.len() as u64 + 1;
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `--diagnostics-report [FILE]`: print the ranking of `FILE`, or of the
/// newest recording. Returns the exit code.
pub fn print_report(file: Option<PathBuf>) -> i32 {
    let Some(file) = file.or_else(latest) else {
        eprintln!("gentle-shell-gpui: no diagnostics recording found");
        return 1;
    };
    match std::fs::read_to_string(&file) {
        Ok(text) => {
            println!("{}\n", file.display());
            print!("{}", model::report(&text));
            0
        }
        Err(error) => {
            eprintln!("gentle-shell-gpui: {}: {error}", file.display());
            1
        }
    }
}
