//! Independent active-break settings, launch clock, and preview.

use std::path::Path;
use std::time::{Duration, Instant};

use gentle_core::hydration::Reminder;
use gpui_kit::*;

use crate::shared::themes;

mod stretch;
#[cfg(test)]
mod tests;

pub use stretch::overlay;

const KEY: &str = "activeBreak";
const DURATION: Duration = Duration::from_secs(6);
// Let the elapsed clock move past the due boundary before computing the next wait.
const SETTLE: Duration = Duration::from_secs(1);

pub struct ActiveBreak {
    start: Instant,
    reminder: Reminder,
    runs: u64,
    playing: Option<u64>,
    _timer: Task<()>,
    _end: Task<()>,
}

impl Global for ActiveBreak {}

fn read(path: Option<&Path>) -> Reminder {
    let saved = path.and_then(|path| themes::read_key(path, KEY));
    Reminder {
        enabled: saved
            .as_ref()
            .and_then(|value| value["enabled"].as_bool())
            .unwrap_or(false),
        ..saved.as_ref().map(Reminder::from_json).unwrap_or_default()
    }
}

fn save(path: &Path, reminder: Reminder) -> std::io::Result<()> {
    themes::write_key(path, KEY, reminder.to_json())
}

pub fn init(cx: &mut App) {
    start(read(themes::store_path().as_deref()), cx);
}

fn start(reminder: Reminder, cx: &mut App) {
    cx.set_global(ActiveBreak {
        start: Instant::now(),
        reminder,
        runs: 0,
        playing: None,
        _timer: Task::ready(()),
        _end: Task::ready(()),
    });
    schedule(cx);
}

pub fn reminder(cx: &App) -> Reminder {
    cx.global::<ActiveBreak>().reminder
}

pub fn playing(cx: &App) -> Option<u64> {
    cx.global::<ActiveBreak>().playing
}

pub fn set(reminder: Reminder, cx: &mut App) {
    cx.update_global::<ActiveBreak, _>(|state, _| state.reminder = reminder);
    schedule(cx);
    if let Some(path) = themes::store_path()
        && let Err(err) = save(&path, reminder)
    {
        eprintln!("gentle-shell-gpui: could not save the active break reminder: {err}");
    }
}

/// Previewing (even while off) replaces only the current run, not the timer.
pub fn show(cx: &mut App) {
    let run = cx.update_global::<ActiveBreak, _>(|state, _| {
        state.runs += 1;
        state.playing = Some(state.runs);
        state.runs
    });
    let end = cx.spawn(async move |cx| {
        cx.background_executor().timer(DURATION).await;
        cx.update(|cx| {
            if playing(cx) == Some(run) {
                cx.update_global::<ActiveBreak, _>(|state, _| state.playing = None);
            }
        });
    });
    cx.global_mut::<ActiveBreak>()._end = end;
}

fn schedule(cx: &mut App) {
    cx.global_mut::<ActiveBreak>()._timer = Task::ready(());
    if !reminder(cx).enabled {
        return;
    }
    let timer = cx.spawn(async move |cx| {
        loop {
            let Some(wait) = cx.update(|cx| {
                let state = cx.global::<ActiveBreak>();
                state.reminder.until_next(state.start.elapsed())
            }) else {
                return;
            };
            cx.background_executor().timer(wait).await;
            cx.update(show);
            cx.background_executor().timer(SETTLE).await;
        }
    });
    cx.global_mut::<ActiveBreak>()._timer = timer;
}
