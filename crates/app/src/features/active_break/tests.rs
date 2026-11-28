use super::{DURATION, playing, read, reminder, save, show, start};
use crate::shared::themes;
use gentle_core::hydration::Reminder;
use gpui::TestAppContext;
use serde_json::json;
use std::{path::PathBuf, time::Duration};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("active-break-{}-{nanos}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn settings(&self) -> PathBuf {
        self.0.join("settings.json")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn active_break_settings_are_independent_and_default_to_off() {
    let scratch = Scratch::new();
    let path = scratch.settings();
    let off = Reminder {
        enabled: false,
        every_minutes: 60,
    };
    let water = json!({"enabled": true, "everyMinutes": 90});
    assert_eq!(read(Some(&path)), off);
    themes::write_key(&path, "hydration", water.clone()).unwrap();
    assert_eq!(read(Some(&path)), off);
    assert_eq!(themes::read_key(&path, "hydration"), Some(water.clone()));
    assert_eq!(themes::read_key(&path, "activeBreak"), None);

    for every_minutes in [20, 30, 45, 60, 90, 120] {
        let choice = Reminder {
            enabled: true,
            every_minutes,
        };
        save(&path, choice).unwrap();
        assert_eq!(read(Some(&path)), choice);
        assert_eq!(themes::read_key(&path, "hydration"), Some(water.clone()));
        assert_eq!(
            themes::read_key(&path, "activeBreak"),
            Some(choice.to_json())
        );
    }
    themes::write_key(&path, "activeBreak", json!({"everyMinutes": 20})).unwrap();
    assert_eq!(
        read(Some(&path)),
        Reminder {
            every_minutes: 20,
            ..off
        }
    );
    themes::write_key(
        &path,
        "activeBreak",
        json!({"enabled": true, "everyMinutes": 25}),
    )
    .unwrap();
    assert_eq!(
        read(Some(&path)),
        Reminder {
            enabled: true,
            ..off
        }
    );
}

#[gpui::test]
fn active_break_can_preview_and_replay_while_off_without_automatic_runs(cx: &mut TestAppContext) {
    cx.update(|cx| start(read(None), cx));
    cx.run_until_parked();
    cx.executor()
        .advance_clock(Duration::from_secs(2 * 60 * 60));
    assert_eq!(cx.update(|cx| playing(cx)), None);

    cx.update(show);
    let first = cx.update(|cx| playing(cx)).unwrap();
    assert_eq!(first, 1, "no automatic runs while off");
    cx.run_until_parked();
    cx.executor().advance_clock(DURATION / 2);
    cx.update(show);
    let second = cx.update(|cx| playing(cx)).unwrap();
    assert_ne!(first, second);
    cx.run_until_parked();
    cx.executor().advance_clock(DURATION / 2);
    assert_eq!(cx.update(|cx| playing(cx)), Some(second));
    cx.executor().advance_clock(DURATION / 2);
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| playing(cx)), None);
    assert!(!cx.update(|cx| reminder(cx)).enabled);
}

#[gpui::test]
fn active_break_preview_does_not_postpone_the_first_scheduled_run(cx: &mut TestAppContext) {
    let enabled = Reminder {
        enabled: true,
        every_minutes: 20,
    };
    cx.update(|cx| start(enabled, cx));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(10 * 60));
    assert_eq!(cx.update(|cx| playing(cx)), None);
    cx.update(show);
    let preview = cx.update(|cx| playing(cx)).unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(DURATION);
    assert_eq!(cx.update(|cx| playing(cx)), None);

    // Only the first wake is checked: subsequent waits use real Instant elapsed.
    cx.executor()
        .advance_clock(Duration::from_secs(10 * 60) - DURATION);
    let scheduled = cx.update(|cx| playing(cx)).unwrap();
    assert_ne!(scheduled, preview);
    assert_eq!(cx.update(|cx| reminder(cx)), enabled);
}

#[gpui::test]
fn active_break_retime_and_disable_cancel_pending_wakes(cx: &mut TestAppContext) {
    let minute = Duration::from_secs(60);
    cx.update(|cx| {
        start(
            Reminder {
                enabled: true,
                every_minutes: 20,
            },
            cx,
        );
    });
    cx.run_until_parked();
    cx.executor().advance_clock(10 * minute);

    cx.update(|cx| {
        cx.global_mut::<super::ActiveBreak>().reminder.every_minutes = 30;
        super::schedule(cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(11 * minute);
    assert_eq!(cx.update(|cx| cx.global::<super::ActiveBreak>().runs), 0);

    cx.update(|cx| {
        cx.global_mut::<super::ActiveBreak>().reminder.enabled = false;
        super::schedule(cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(120 * minute);
    assert_eq!(cx.update(|cx| cx.global::<super::ActiveBreak>().runs), 0);
    assert_eq!(cx.update(|cx| playing(cx)), None);
}

#[gpui::test]
fn active_break_two_automatic_runs_each_expire(cx: &mut TestAppContext) {
    cx.update(|cx| {
        start(
            Reminder {
                enabled: true,
                every_minutes: 20,
            },
            cx,
        );
    });
    cx.run_until_parked();

    for run in [1, 2] {
        // Observe each playback within its lifetime, without claiming the
        // exact wake time: the schedule reads real elapsed time, not GPUI's clock.
        for _ in 0..=601 {
            if cx.update(|cx| playing(cx)).is_some() {
                break;
            }
            cx.executor().advance_clock(Duration::from_secs(2));
        }
        assert_eq!(cx.update(|cx| playing(cx)), Some(run));
        cx.run_until_parked();
        cx.executor().advance_clock(DURATION);
        assert_eq!(cx.update(|cx| playing(cx)), None);
    }
}
