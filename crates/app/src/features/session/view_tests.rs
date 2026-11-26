//! Interaction tests: a real [`SessionView`] (no backend started) in a
//! headless window, its Desktop channel fed the records Gentle Shell would
//! send, driven by simulated clicks.

// Not a glob: GPUI exports a `test` attribute that would shadow `#[test]`.
use std::cell::RefCell;
use std::rc::Rc;

use gentle_core::commands::Screen;
use gpui_kit::component::input::Position;
use gpui_kit::{
    AppContext as _, Context, Entity, FileDropEvent, InteractiveElement as _, IntoElement,
    Modifiers, MouseButton, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestAppContext, VisualTestContext, Window,
    component::{WindowExt as _, h_flex, v_flex},
    div, point, px,
};
use pi_rpc::desktop_rpc::{PROMPT_COMMAND, SCHEMA, WIDGET_KEY};
use pi_rpc::{UiMethod, UiRequest};
use serde_json::{Value, json};

use super::attachments::{AttachedFile, Attachment, Loaded};
use super::{AgentsTab, SessionView};
use crate::features::files::FileDrag;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10];

fn widget(record: Value) -> UiRequest {
    UiRequest {
        id: "ui-1".into(),
        timeout: None,
        method: UiMethod::SetWidget {
            key: WIDGET_KEY.into(),
            lines: Some(vec![record.to_string()]),
            placement: None,
        },
    }
}

fn response(request_id: &str, data: Value) -> UiRequest {
    widget(json!({
        "schema": SCHEMA, "kind": "response", "requestId": request_id, "ok": true, "data": data,
    }))
}

/// A tab in a headless window that never starts a backend (drawing a tab
/// starts one otherwise).
fn tab(cx: &mut TestAppContext) -> (Entity<SessionView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let cwd = std::env::temp_dir();
    cx.add_window_view(|window, cx| {
        let mut view = SessionView::new(cwd, None, window, cx, false);
        view.backend.started = true;
        view.usage.context_test_ready = true;
        view.usage.context_stats_received = true;
        view
    })
}

/// The same tab in a window with the app's root, so a warning it raises is
/// shown (and can be counted): [`warnings`].
fn rooted_tab(cx: &mut TestAppContext) -> (Entity<SessionView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let cwd = std::env::temp_dir();
    let mut built = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let tab = cx.new(|cx| {
            let mut view = SessionView::new(cwd, None, window, cx, false);
            view.backend.started = true;
            view
        });
        built = Some(tab.clone());
        gpui_kit::base::Root::new(tab, window, cx)
    });
    (built.expect("the tab was built"), cx)
}

/// The warnings the window is showing: what a refusal raises.
fn warnings(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.notifications(cx).len())
}

/// What the composer holds.
fn composer_text(view: &Entity<SessionView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string())
}

/// Put `text` in the composer, as typing it would.
fn type_text(view: &Entity<SessionView>, cx: &mut VisualTestContext, text: &str) {
    view.update_in(cx, |view, window, cx| {
        view.composer
            .update(cx, |state, cx| state.set_value(text, window, cx));
    });
    cx.run_until_parked();
}

/// Press Enter: the send path the composer's key and its button share.
fn press_enter(view: &Entity<SessionView>, cx: &mut VisualTestContext) {
    view.update_in(cx, |view, window, cx| view.submit(window, cx));
}

/// A tab whose channel is probed with `methods`, and whose job and helper
/// lists hold one running job (`job-1`) and one helper (`agent-1`) the
/// `gentle-agents` widget has no thread for.
fn open<'a>(
    methods: &[&str],
    cx: &'a mut TestAppContext,
) -> (Entity<SessionView>, &'a mut VisualTestContext) {
    let (view, cx) = tab(cx);
    let mut all = vec![
        "capabilities",
        "get_snapshot",
        "list_jobs",
        "list_subagents",
    ];
    all.extend_from_slice(methods);
    view.update(cx, |view, _| {
        let desktop = &mut view.desktop;
        let probe = desktop.ready(true).sends.remove(0);
        let snapshot = desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": all, "events": ["jobs_update", "agents_update"],
                }),
            ))
            .sends
            .remove(0);
        desktop.ui(&response(
            snapshot.request.request_id(),
            json!({"schema": SCHEMA, "sequence": 1, "state": {}}),
        ));
        let agents = desktop.supervision_seed().sends.remove(0);
        desktop.supervision_apply(&response(
            agents.request.request_id(),
            json!({"agents": [{
                "id": "agent-1", "agent": "worker", "status": "running",
                "model": "anthropic/claude-sonnet-4", "lastStep": "writing",
                "usage": {"total": 100, "cost": 0.01},
            }]}),
        ));
        let jobs = desktop.supervision_seed().sends.remove(0);
        desktop.supervision_apply(&response(
            jobs.request.request_id(),
            json!({"jobs": [{"id": "job-1", "label": "npm test", "status": "running", "startedAt": 1}]}),
        ));
        assert!(desktop.jobs_ready() && desktop.subagents_ready());
    });
    (view, cx)
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("nothing drawn as {selector}"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// Answer the output read the slice owes now, as Gentle Shell would.
fn answer(view: &Entity<SessionView>, cx: &mut VisualTestContext, data: Value) {
    view.update(cx, |view, cx| {
        let sends = view.desktop.output_pump().sends;
        let id = sends[0].request.request_id().to_string();
        view.desktop.output_apply(&response(&id, data));
        cx.notify();
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_job_row_folds_its_output_open_and_shut(cx: &mut TestAppContext) {
    let (view, cx) = open(&["get_job_output"], cx);
    view.update(cx, |view, cx| view.open_agents_panel(AgentsTab::Jobs, cx));
    assert!(drawn(cx, "job-row-job-1"));
    assert!(!drawn(cx, "job-output"));

    click(cx, "job-row-job-1");
    // The tab has no backend, so the first read was dropped; the well shows
    // that it is reading, and the next owed read gets its answer.
    assert!(drawn(cx, "job-output"));
    view.update(cx, |view, _| {
        assert!(view.desktop.output_mut().tick_job());
    });
    answer(
        &view,
        cx,
        json!({
            "job": {"id": "job-1", "label": "npm test", "status": "running", "startedAt": 1},
            "output": "✓ lint\nrunning tests",
            "truncated": true,
        }),
    );
    view.read_with(cx, |view, _| {
        let job = view.desktop.output().job().unwrap();
        assert_eq!(job.output.as_ref().unwrap().output, "✓ lint\nrunning tests");
    });
    assert!(drawn(cx, "job-output"));

    // Clicking it again folds it and stops reading.
    click(cx, "job-row-job-1");
    assert!(!drawn(cx, "job-output"));
    view.read_with(cx, |view, _| assert!(view.desktop.output().job().is_none()));
}

#[gpui::test]
fn without_get_job_output_a_job_row_stays_as_it_was(cx: &mut TestAppContext) {
    let (view, cx) = open(&[], cx);
    view.update(cx, |view, cx| view.open_agents_panel(AgentsTab::Jobs, cx));
    click(cx, "job-row-job-1");
    assert!(!drawn(cx, "job-output"));
    view.read_with(cx, |view, _| assert!(view.desktop.output().job().is_none()));
}

#[gpui::test]
fn the_activity_tab_reads_a_thread_the_widget_did_not_publish(cx: &mut TestAppContext) {
    let (view, cx) = open(&["get_subagent_thread"], cx);
    view.update(cx, |view, cx| {
        view.open_agents_panel(AgentsTab::Activity, cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, _| {
        let thread = view.desktop.output().thread().expect("watched on open");
        assert_eq!(thread.id, "agent-1");
        // No backend here: the dropped first read is asked again, as an
        // `agents_update` would.
        view.desktop.output_mut().refresh_thread();
    });
    assert!(!drawn(cx, "agent-thread"));
    answer(
        &view,
        cx,
        json!({
            "id": "agent-1", "version": 3, "dropped": 0, "truncated": false,
            "items": [{"kind": "text", "text": "Mapping the repository layout."}],
        }),
    );
    assert!(drawn(cx, "agent-thread"));

    // Leaving the tab stops reading it.
    view.update(cx, |view, cx| view.close_agents(cx));
    view.read_with(cx, |view, _| {
        assert!(view.desktop.output().thread().is_none())
    });
}

#[gpui::test]
fn without_get_subagent_thread_the_activity_tab_keeps_its_empty_state(cx: &mut TestAppContext) {
    let (view, cx) = open(&[], cx);
    view.update(cx, |view, cx| {
        view.open_agents_panel(AgentsTab::Activity, cx)
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(view.desktop.output().thread().is_none())
    });
    assert!(!drawn(cx, "agent-thread"));
}

/// The session feed's sections drive the todo island and the command menu;
/// a feed-listed Pi built-in that only works in the terminal is never sent.
#[gpui::test]
fn the_session_feed_drives_the_todo_list_and_the_command_menu(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        let probe = view.desktop.ready(true).sends.remove(0);
        let snapshot = view
            .desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": ["capabilities", "get_snapshot"], "events": ["session_snapshot"],
                }),
            ))
            .sends
            .remove(0);
        view.desktop.ui(&response(
            snapshot.request.request_id(),
            json!({"schema": SCHEMA, "sequence": 1, "state": {"session": {
                "context": {"percent": 61.0, "tokens": 122000, "window": 200000},
                "todo": {
                    "items": [{"id": 1, "title": "Wire the feed", "state": "in_progress", "note": null}],
                    "progress": {"done": 0, "total": 1, "open": 1}, "current": "Wire the feed", "truncated": false,
                },
                "activity": {"state": "idle", "streaming": false, "phase": null, "phaseLabel": null,
                             "blockedLabel": null, "agents": 0, "jobs": 0, "label": "Idle"},
                "pendingQuestion": null,
                "commands": {"items": [
                    {"name": "compact", "description": "Pi compact", "args": null, "terminal": true, "source": "builtin"},
                    {"name": "model", "description": "Select model", "args": null, "terminal": true, "source": "builtin"},
                ], "truncated": false},
            }}}),
        ));
        view.sync_sections(cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, cx| {
        let titles: Vec<&str> = view.todo.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Wire the feed"]);
        assert_eq!(view.usage.context_percent, Some(61.0));

        let commands = view.commands();
        let compact = commands.iter().find(|c| c.name == "compact").unwrap();
        assert!(!compact.terminal, "the app's own /compact wins");
        let model = commands.iter().find(|c| c.name == "model").unwrap();
        assert!(model.terminal);

        let before = view.transcript.items.len();
        view.run_prompt("/model".into(), cx);
        assert_eq!(view.transcript.items.len(), before + 1);
        assert!(matches!(
            view.transcript.items.last(),
            Some(gentle_core::transcript::Item::Notice { text, .. })
                if text == "/model only works in Gentle Shell's terminal."
        ));
    });
}

/// A tab under the window's text-selection layer, as the app's Root draws it.
struct Selectable(Entity<SessionView>);

impl gpui_kit::Render for Selectable {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div()
            .size_full()
            .child(gpui_kit::base::TextSelectionLayer)
            .child(self.0.clone())
    }
}

/// A message you sent can be selected with the mouse, like an answer, and
/// what is selected is its text.
#[gpui::test]
fn context_feed_updates_exact_usage_and_clears_it(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        let probe = view.desktop.ready(true).sends.remove(0);
        let snapshot = view
            .desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": ["capabilities", "get_snapshot"], "events": ["session_snapshot"],
                }),
            ))
            .sends
            .remove(0);
        view.desktop.ui(&response(
            snapshot.request.request_id(),
            json!({
                "schema": SCHEMA, "sequence": 1, "state": {"session": {
                    "context": {"percent": 61.0, "tokens": 122000, "window": 200000}
                }}
            }),
        ));
        view.sync_sections(cx);
        assert_eq!(
            (
                view.usage.context_percent,
                view.usage.context_tokens,
                view.usage.context_window
            ),
            (Some(61.0), Some(122000), Some(200000))
        );

        let event = |seq, sections| {
            widget(json!({
                "schema": SCHEMA, "kind": "event", "seq": seq,
                "event": "session_snapshot", "data": {"sections": sections},
            }))
        };
        view.desktop.ui(&event(2, json!({})));
        view.sync_sections(cx);
        assert_eq!(
            (
                view.usage.context_percent,
                view.usage.context_tokens,
                view.usage.context_window
            ),
            (Some(61.0), Some(122000), Some(200000))
        );

        view.desktop.ui(&event(
            3,
            json!({"context": {"percent": 25.0, "tokens": null, "window": 100000}}),
        ));
        view.sync_sections(cx);
        assert_eq!(
            (
                view.usage.context_percent,
                view.usage.context_tokens,
                view.usage.context_window
            ),
            (Some(25.0), None, Some(100000))
        );

        view.desktop.ui(&event(4, json!({"context": null})));
        view.sync_sections(cx);
        assert_eq!(
            (
                view.usage.context_percent,
                view.usage.context_tokens,
                view.usage.context_window
            ),
            (None, None, None)
        );
    });
}

#[gpui::test]
fn context_ring_is_hidden_during_startup_even_with_context_data(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_test_ready = false;
        view.usage.context_stats_received = false;
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring"));
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(61.0);
        view.usage.context_tokens = Some(122000);
        view.usage.context_stats_received = true;
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring"));
    view.update(cx, |view, cx| {
        view.usage.reset_context();
        view.usage.context_test_ready = true;
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring"));
    view.update(cx, |view, cx| {
        assert_eq!(view.usage.context_percent, None);
        assert_eq!(view.usage.context_tokens, None);
        // A successful empty-session stats response has no percentage.
        view.usage.context_stats_received = true;
        cx.notify();
    });
    assert!(drawn(cx, "context-ring"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
}

#[gpui::test]
fn post_compaction_estimate_is_shown_approximately(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, _| {
        view.usage.context_window = Some(200000);
        view.usage.context_tokens = Some(120000);
        view.usage.context_percent = Some(60.0);
    });
    click(cx, "context-ring");
    view.update_in(cx, |view, window, cx| {
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
            }))],
            window,
            cx,
        );
    });
    assert!(drawn(cx, "context-ring-used-tokens-~40k"));
    assert!(drawn(cx, "context-ring-usage-~20%"));
    assert!(!drawn(cx, "context-ring-used-tokens-40k"));
    click(cx, "context-ring");
    let at = cx.debug_bounds("context-ring").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(!drawn(cx, "context-ring-overview-~40k (~20%)"));
}

#[gpui::test]
fn compaction_without_estimate_restores_interaction_without_inventing_usage(
    cx: &mut TestAppContext,
) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_window = Some(200000);
        cx.notify();
    });
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
    view.update_in(cx, |view, window, cx| {
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({"type": "compaction_start"}))],
            window,
            cx,
        );
    });
    assert!(!drawn(cx, "context-compact-control"));
    assert!(drawn(cx, "context-compaction-orbit"));
    click(cx, "context-ring");
    assert!(!drawn(cx, "context-compact-control"));
    view.update_in(cx, |view, window, cx| {
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {}
            }))],
            window,
            cx,
        );
    });
    assert!(!drawn(cx, "context-compaction-orbit"));
    view.read_with(cx, |view, _| {
        assert_eq!(view.usage.context_tokens, None);
        assert_eq!(view.usage.context_percent, None);
        assert_eq!(view.usage.context_window, Some(200000));
        assert!(!view.usage.context_estimated);
    });
    let at = cx.debug_bounds("context-ring").unwrap().center();
    cx.simulate_mouse_move(
        gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
        None,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(!drawn(cx, "context-ring-overview-hint"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
    assert!(drawn(cx, "context-auto-compaction"));
    assert!(!drawn(cx, "context-ring-usage-0%"));
    assert!(!drawn(cx, "context-ring-used-tokens-0"));
}

#[gpui::test]
fn post_compaction_estimate_handles_missing_failure_and_stale_stats(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update_in(cx, |view, window, cx| {
        view.usage.context_window = None;
        let old = view.usage.context_revision;
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
            }))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_tokens, Some(40000));
        assert_eq!(view.usage.context_percent, None);
        view.usage.context_window = Some(0);
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
            }))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_percent, None);
        view.apply_stats_at(old, Ok(json!({"contextUsage": {"percent": 80.0}})), cx);
        assert_eq!(view.usage.context_percent, None);
        view.apply_stats_at(
            view.usage.context_revision,
            Ok(json!({"contextUsage": {"percent": null}})),
            cx,
        );
        assert_eq!(view.usage.context_tokens, Some(40000));
        for event in [
            json!({"errorMessage": "failed", "result": {"estimatedTokensAfter": 1}}),
            json!({"aborted": true, "result": {"estimatedTokensAfter": 2}}),
            json!({"cancelled": true, "result": {"estimatedTokensAfter": 3}}),
            json!({"success": false, "result": {"estimatedTokensAfter": 4}}),
            json!({"result": {"estimatedTokensAfter": null}}),
        ] {
            let mut event = event;
            event["type"] = json!("compaction_end");
            view.handle_batch(vec![pi_rpc::Incoming::Event(event)], window, cx);
            assert_eq!(view.usage.context_tokens, Some(40000));
        }
        view.apply_stats_at(
            view.usage.context_revision,
            Ok(json!({"contextUsage": {"percent": 30.0}})),
            cx,
        );
        assert_eq!(view.usage.context_percent, Some(30.0));
        assert_eq!(view.usage.context_tokens, None);
        assert!(!view.usage.context_estimated);
    });
}

#[gpui::test]
fn post_compaction_estimate_survives_empty_context(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update_in(cx, |view, window, cx| {
        let probe = view.desktop.ready(true).sends.remove(0);
        let snapshot = view
            .desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": ["capabilities", "get_snapshot"], "events": ["session_snapshot"]
                }),
            ))
            .sends
            .remove(0);
        view.desktop.ui(&response(
            snapshot.request.request_id(),
            json!({
                "schema": SCHEMA, "sequence": 1, "state": {"session": {
                    "context": {"percent": 60.0, "tokens": 163200, "window": 272000}
                }}
            }),
        ));
        view.sync_sections(cx);
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 62800}
            }))],
            window,
            cx,
        );
        for (seq, context) in [
            (
                2,
                json!({"percent": null, "tokens": null, "window": 272000}),
            ),
            (3, json!({"window": 272000})),
        ] {
            view.handle_batch(
                vec![pi_rpc::Incoming::Ui(widget(json!({
                    "schema": SCHEMA, "kind": "event", "seq": seq,
                    "event": "session_snapshot", "data": {"sections": {"context": context}}
                })))],
                window,
                cx,
            );
        }
    });
    click(cx, "context-ring");
    assert!(drawn(cx, "context-ring-used-tokens-~62.8k"));
    assert!(drawn(cx, "context-ring-usage-~23%"));
    click(cx, "context-ring");
    let at = cx.debug_bounds("context-ring").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(!drawn(cx, "context-ring-overview-~62.8k (~23%)"));
    view.update_in(cx, |view, window, cx| {
        for (seq, context, expected_percent) in [
            (4, json!({"window": 314000}), Some(20.0)),
            (5, json!({"window": 0}), None),
            (6, json!({"window": null}), None),
        ] {
            view.handle_batch(
                vec![pi_rpc::Incoming::Ui(widget(json!({
                    "schema": SCHEMA, "kind": "event", "seq": seq,
                    "event": "session_snapshot", "data": {"sections": {"context": context}}
                })))],
                window,
                cx,
            );
            assert_eq!(view.usage.context_tokens, Some(62800));
            assert_eq!(view.usage.context_percent, expected_percent);
            assert!(view.usage.context_estimated);
        }
        view.handle_batch(
            vec![pi_rpc::Incoming::Ui(widget(json!({
                "schema": SCHEMA, "kind": "event", "seq": 7,
                "event": "session_snapshot", "data": {"sections": {"context": {
                    "tokens": 50000, "percent": 25.0, "window": 200000
                }}}
            })))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_tokens, Some(50000));
        assert_eq!(view.usage.context_percent, Some(25.0));
        assert!(!view.usage.context_estimated);
    });
}

#[gpui::test]
fn post_compaction_estimate_accepts_current_stats_after_empty_feed(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update_in(cx, |view, window, cx| {
        let probe = view.desktop.ready(true).sends.remove(0);
        let snapshot = view
            .desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": ["capabilities", "get_snapshot"], "events": ["session_snapshot"]
                }),
            ))
            .sends
            .remove(0);
        view.desktop.ui(&response(
            snapshot.request.request_id(),
            json!({
                "schema": SCHEMA, "sequence": 1, "state": {"session": {
                    "context": {"percent": 60.0, "tokens": 120000, "window": 200000}
                }}
            }),
        ));
        view.sync_sections(cx);
        let delta = |seq, sections| {
            widget(json!({
                "schema": SCHEMA, "kind": "event", "seq": seq,
                "event": "session_snapshot", "data": {"sections": sections}
            }))
        };
        for (seq, sections) in [
            (2, json!({})),
            (3, json!({"context": null})),
            (
                4,
                json!({"context": {"tokens": null, "percent": null, "window": 200000}}),
            ),
            (5, json!({"context": {"window": 272000}})),
        ] {
            let stale = view.usage.context_revision;
            view.handle_batch(
                vec![pi_rpc::Incoming::Event(json!({
                    "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
                }))],
                window,
                cx,
            );
            let current = view.usage.context_revision;
            view.handle_batch(vec![pi_rpc::Incoming::Ui(delta(seq, sections))], window, cx);
            view.apply_stats_at(stale, Ok(json!({"contextUsage": {"percent": 80.0}})), cx);
            assert!(view.usage.context_estimated);
            view.apply_stats_at(current, Ok(json!({"contextUsage": {"percent": 30.0}})), cx);
            assert_eq!(view.usage.context_percent, Some(30.0), "sequence {seq}");
            assert_eq!(view.usage.context_tokens, None);
            assert!(!view.usage.context_estimated);
        }
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
            }))],
            window,
            cx,
        );
        let current = view.usage.context_revision;
        view.handle_batch(
            vec![pi_rpc::Incoming::Ui(delta(
                6,
                json!({"context": {
                    "tokens": 50000, "percent": 25.0, "window": 200000
                }}),
            ))],
            window,
            cx,
        );
        view.apply_stats_at(current, Ok(json!({"contextUsage": {"percent": 30.0}})), cx);
        assert_eq!(view.usage.context_percent, Some(25.0));
        assert_eq!(view.usage.context_tokens, Some(50000));
        assert!(!view.usage.context_estimated);
    });
}

#[gpui::test]
fn post_compaction_estimate_survives_unrelated_feed_delta(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update_in(cx, |view, window, cx| {
        let probe = view.desktop.ready(true).sends.remove(0);
        let snapshot = view
            .desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": ["capabilities", "get_snapshot"], "events": ["session_snapshot"]
                }),
            ))
            .sends
            .remove(0);
        view.desktop.ui(&response(
            snapshot.request.request_id(),
            json!({
                "schema": SCHEMA, "sequence": 1, "state": {"session": {
                    "context": {"percent": 60.0, "tokens": 120000, "window": 200000}
                }}
            }),
        ));
        view.sync_sections(cx);
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
            }))],
            window,
            cx,
        );
        let delta = |seq, sections| {
            widget(json!({
                "schema": SCHEMA, "kind": "event", "seq": seq,
                "event": "session_snapshot", "data": {"sections": sections}
            }))
        };
        view.handle_batch(vec![pi_rpc::Incoming::Ui(delta(2, json!({})))], window, cx);
        assert_eq!(view.usage.context_tokens, Some(40000));
        assert!(view.usage.context_estimated);
        view.handle_batch(
            vec![pi_rpc::Incoming::Ui(delta(
                3,
                json!({
                    "context": {"percent": 25.0, "tokens": 50000, "window": 200000}
                }),
            ))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_tokens, Some(50000));
        assert_eq!(view.usage.context_percent, Some(25.0));
        assert!(!view.usage.context_estimated);
        view.handle_batch(
            vec![pi_rpc::Incoming::Ui(delta(4, json!({"context": null})))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_tokens, None);
        assert_eq!(view.usage.context_percent, None);
    });
}

#[gpui::test]
fn post_compaction_estimate_rejects_pre_compaction_snapshot(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update_in(cx, |view, window, cx| {
        view.usage.context_window = Some(200000);
        let probe = view.desktop.ready(true).sends.remove(0);
        let snapshot = view
            .desktop
            .ui(&response(
                probe.request.request_id(),
                json!({
                    "schema": SCHEMA, "command": PROMPT_COMMAND, "widget": WIDGET_KEY,
                    "methods": ["capabilities", "get_snapshot"], "events": ["session_snapshot"]
                }),
            ))
            .sends
            .remove(0);
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "compaction_end", "result": {"estimatedTokensAfter": 40000}
            }))],
            window,
            cx,
        );
        view.handle_batch(
            vec![pi_rpc::Incoming::Ui(response(
                snapshot.request.request_id(),
                json!({
                    "schema": SCHEMA, "sequence": 1, "state": {"session": {
                        "context": {"percent": 60.0, "tokens": 120000, "window": 200000}
                    }}
                }),
            ))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_tokens, Some(40000));
        assert_eq!(view.usage.context_percent, Some(20.0));
        assert!(view.usage.context_estimated);
        let fresh = view.desktop.resync().remove(0);
        view.handle_batch(
            vec![pi_rpc::Incoming::Ui(response(
                fresh.request.request_id(),
                json!({
                    "schema": SCHEMA, "sequence": 2, "state": {"session": {
                        "context": {"percent": 25.0, "tokens": 50000, "window": 200000}
                    }}
                }),
            ))],
            window,
            cx,
        );
        assert_eq!(view.usage.context_tokens, Some(50000));
        assert_eq!(view.usage.context_percent, Some(25.0));
        assert!(!view.usage.context_estimated);
    });
}

#[gpui::test]
fn post_compaction_estimate_preserves_normal_stats(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.apply_stats(
            Ok(json!({"cost": 0.5, "contextUsage": {"percent": 61.0}})),
            cx,
        );
        assert_eq!(view.usage.cost, 0.5);
        assert_eq!(view.usage.context_percent, Some(61.0));
        assert!(!view.usage.context_estimated);
    });
    click(cx, "context-ring");
    assert!(drawn(cx, "context-ring-usage-61%"));
}

#[gpui::test]
fn context_ring_failed_stats_then_refresh(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.reset_context();
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring"));
    view.update(cx, |view, cx| {
        view.apply_stats(Err(anyhow::anyhow!("stats failed")), cx);
        cx.notify();
    });
    assert!(drawn(cx, "context-ring"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
    assert!(drawn(cx, "context-auto-compaction"));
    assert!(!drawn(cx, "context-ring-usage-0%"));
    view.update(cx, |view, cx| {
        view.apply_stats(
            Ok(json!({"cost": 0.5, "contextUsage": {"percent": 61.0}})),
            cx,
        );
        cx.notify();
    });
    assert!(drawn(cx, "context-ring-usage-61%"));
    view.update(cx, |view, _| assert_eq!(view.usage.cost, 0.5));
}

#[gpui::test]
fn context_ring_empty_stats_response(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.reset_context();
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring"));
    view.update(cx, |view, cx| {
        view.apply_stats(Ok(json!({})), cx);
        cx.notify();
    });
    assert!(drawn(cx, "context-ring"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
    assert!(!drawn(cx, "context-ring-usage-0%"));
}

#[gpui::test]
fn context_ring_shows_model_window_without_context_feed(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.models.current = super::state::ModelInfo::parse(&json!({
            "id": "legacy-model", "contextWindow": 200000
        }));
        view.apply_stats(Ok(json!({"contextUsage": {"percent": 25.0}})), cx);
        cx.notify();
    });
    let at = cx
        .debug_bounds("context-ring")
        .expect("ring drawn")
        .center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(!drawn(cx, "context-ring-overview-25%"));
    assert!(!drawn(cx, "context-ring-overview-window-200k"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-ring-context-window-200k"));
    assert!(drawn(cx, "context-ring-usage-25%"));
    assert!(!drawn(cx, "context-ring-used-tokens-50k"));
}

#[gpui::test]
fn context_ring_tooltip_survives_live_usage_updates(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(5.0);
        view.usage.context_tokens = Some(48100);
        view.usage.context_window = Some(200000);
        cx.notify();
    });
    assert!(drawn(cx, "context-ring"));
    let at = cx.debug_bounds("context-ring").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    view.update(cx, |view, _| assert!(view.usage.context_ring_hovered));
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(drawn(cx, "context-ring-tooltip"));
    assert!(!drawn(cx, "context-ring-overview-48.1k (5%)"));
    assert!(!drawn(cx, "context-ring-overview-window-200k"));
    view.update(cx, |view, cx| {
        view.usage.context_tokens = None;
        view.usage.context_percent = Some(25.0);
        cx.notify();
    });
    assert!(drawn(cx, "context-ring-tooltip"));
    assert!(!drawn(cx, "context-ring-overview-25%"));
    assert!(!drawn(cx, "context-ring-overview-48.1k (5%)"));
    cx.simulate_mouse_move(
        gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
        None,
        Modifiers::none(),
    );
    view.update(cx, |view, _| assert!(!view.usage.context_ring_hovered));
}

#[gpui::test]
fn context_ring_tooltip_returns_after_toggle_and_outside_click(cx: &mut TestAppContext) {
    let (_, cx) = tab(cx);
    assert!(drawn(cx, "context-ring"));
    let at = cx.debug_bounds("context-ring").unwrap().center();
    let outside = gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.));
    for outside_click in [false, true] {
        cx.simulate_mouse_move(outside, None, Modifiers::none());
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        assert!(drawn(cx, "context-ring-tooltip"));
        assert!(!drawn(cx, "context-ring-overview-hint"));
        click(cx, "context-ring");
        assert!(drawn(cx, "context-compact-control"));
        assert!(!drawn(cx, "context-ring-tooltip"));
        assert!(!drawn(cx, "context-ring-overview-hint"));
        cx.simulate_mouse_move(outside, None, Modifiers::none());
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        assert!(!drawn(cx, "context-ring-tooltip"));
        assert!(!drawn(cx, "context-ring-overview-hint"));
        if outside_click {
            cx.simulate_click(outside, Modifiers::none());
        } else {
            click(cx, "context-ring");
        }
        assert!(!drawn(cx, "context-compact-control"));
        cx.simulate_mouse_move(outside, None, Modifiers::none());
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        assert!(drawn(cx, "context-ring-tooltip"));
        assert!(!drawn(cx, "context-ring-overview-hint"));
    }
}

#[gpui::test]
fn context_ring_tooltip_returns_after_escape(cx: &mut TestAppContext) {
    let (_, cx) = tab(cx);
    assert!(drawn(cx, "context-ring"));
    let at = cx.debug_bounds("context-ring").unwrap().center();
    let outside = gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(drawn(cx, "context-ring-tooltip"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(!drawn(cx, "context-ring-tooltip"));
    cx.simulate_keystrokes("escape");
    assert!(!drawn(cx, "context-compact-control"));
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(drawn(cx, "context-ring-tooltip"));
    assert!(!drawn(cx, "context-ring-overview-hint"));
}

#[gpui::test]
fn context_ring_has_no_hover_popup_and_details_have_progress(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(62.0);
        cx.notify();
    });
    let at = cx.debug_bounds("context-ring").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(!drawn(cx, "context-ring-overview-hint"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-ring-progress"));
}

#[gpui::test]
fn context_ring_tooltip_and_advice_spacing(cx: &mut TestAppContext) {
    let (_, cx) = tab(cx);
    let at = cx.debug_bounds("context-ring").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    assert!(drawn(cx, "context-ring-tooltip"));
    assert!(!drawn(cx, "context-ring-overview-hint"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-compact-control"));
    assert!(drawn(cx, "context-auto-compaction"));
    assert!(!drawn(cx, "context-ring-advice-divider"));
    let progress = cx.debug_bounds("context-ring-progress").unwrap();
    let advice = cx
        .debug_bounds("Compacting summarizes the conversation to free context.")
        .unwrap();
    assert!(advice.top() >= progress.bottom());
}

#[gpui::test]
fn context_ring_progress_tracks_live_estimated_and_missing_usage(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(69.0);
        cx.notify();
    });
    click(cx, "context-ring");
    for (percent, selector, label) in [
        (69.0, "context-progress-success", "context-ring-usage-~69%"),
        (70.0, "context-progress-warning", "context-ring-usage-~70%"),
        (90.0, "context-progress-danger", "context-ring-usage-~90%"),
    ] {
        view.update(cx, |view, cx| {
            view.usage.context_percent = Some(percent);
            view.usage.context_estimated = true;
            cx.notify();
        });
        assert!(drawn(cx, selector));
        assert!(drawn(cx, label));
        let track = cx.debug_bounds("context-ring-progress").unwrap();
        let fill = cx.debug_bounds("context-progress-fill").unwrap();
        let ratio = f32::from(fill.size.width) / f32::from(track.size.width);
        assert!((ratio - percent as f32 / 100.).abs() < 0.02);
    }
    view.update(cx, |view, cx| {
        view.usage.context_percent = None;
        cx.notify();
    });
    assert!(!drawn(cx, "context-progress-fill"));
}

#[gpui::test]
fn context_ring_opens_existing_compaction_controls(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(35.0);
        cx.notify();
    });
    assert!(drawn(cx, "context-ring"));
    click(cx, "context-ring");
    assert!(drawn(cx, "context-auto-compaction"));
    assert!(drawn(cx, "context-compact-control"));
}

#[gpui::test]
fn context_ring_details_update_live_and_hide_missing_values(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(61.0);
        view.usage.context_tokens = Some(122000);
        view.usage.context_window = Some(200000);
        cx.notify();
    });
    click(cx, "context-ring");
    assert!(drawn(cx, "context-ring-used-tokens-122k"));
    assert!(drawn(cx, "context-ring-context-window-200k"));
    assert!(drawn(cx, "context-ring-usage-61%"));
    assert!(!drawn(cx, "context-ring-header-count"));
    assert!(drawn(cx, "context-compact-control"));
    assert!(drawn(cx, "context-auto-compaction"));

    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(25.0);
        view.usage.context_tokens = None;
        view.usage.context_window = Some(100000);
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring-used-tokens-122k"));
    assert!(!drawn(cx, "context-ring-context-window-200k"));
    assert!(!drawn(cx, "context-ring-usage-61%"));
    assert!(drawn(cx, "context-ring-context-window-100k"));
    assert!(drawn(cx, "context-ring-usage-25%"));

    view.update(cx, |view, cx| {
        view.usage.context_percent = None;
        view.usage.context_tokens = Some(48100);
        view.usage.context_window = None;
        cx.notify();
    });
    assert!(drawn(cx, "context-ring-used-tokens-48.1k"));
    assert!(!drawn(cx, "context-ring-context-window-100k"));
    assert!(!drawn(cx, "context-ring-usage-25%"));
    view.update(cx, |view, cx| {
        view.usage.context_tokens = None;
        cx.notify();
    });
    assert!(!drawn(cx, "context-ring-used-tokens-48.1k"));
    assert!(!drawn(cx, "context-ring-header-count"));
}

#[gpui::test]
fn context_ring_advice_updates_at_boundaries_and_keeps_controls(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(69.0);
        cx.notify();
    });
    click(cx, "context-ring");
    let low = "Compacting summarizes the conversation to free context.";
    let medium = "Consider compacting or starting a new session.";
    let high = "Context nearly full. Compact or start a new session.";
    for (percent, expected) in [(69.0, low), (70.0, medium), (89.0, medium), (90.0, high)] {
        view.update(cx, |view, cx| {
            view.usage.context_percent = Some(percent);
            cx.notify();
        });
        for text in [low, medium, high] {
            assert_eq!(drawn(cx, text), text == expected, "at {percent}%: {text}");
        }
        assert!(drawn(cx, "Add what to keep with /compact <instructions>."));
        assert!(!drawn(cx, "context-ring-advice-divider"));
        assert!(drawn(cx, "context-compact-control"));
        assert!(drawn(cx, "context-auto-compaction"));
    }
}

#[gpui::test]
fn context_ring_replaces_legacy_context_meter(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.usage.context_percent = Some(95.0);
        cx.notify();
    });
    assert!(!drawn(cx, "legacy-context-chip"));
    assert!(!drawn(cx, "context-percent"));
    assert!(!drawn(cx, "context-window"));
    for end in [
        json!({"type": "compaction_end"}),
        json!({"type": "compaction_end", "errorMessage": "failed"}),
    ] {
        click(cx, "context-ring");
        assert!(drawn(cx, "context-compact-control"));
        view.update_in(cx, |view, window, cx| {
            view.handle_batch(
                vec![pi_rpc::Incoming::Event(json!({"type": "compaction_start"}))],
                window,
                cx,
            );
        });
        assert!(!drawn(cx, "context-compact-control"));
        assert!(drawn(cx, "context-compaction-orbit"));
        click(cx, "context-ring");
        cx.simulate_keystrokes("enter");
        assert!(!drawn(cx, "context-compact-control"));
        let at = cx.debug_bounds("context-ring").unwrap().center();
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        assert!(!drawn(cx, "context-ring-overview-hint"));
        view.update_in(cx, |view, window, cx| {
            view.handle_batch(vec![pi_rpc::Incoming::Event(end)], window, cx);
        });
        assert!(!drawn(cx, "context-compaction-orbit"));
        cx.simulate_mouse_move(
            gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
            None,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        assert!(!drawn(cx, "context-ring-overview-hint"));
        click(cx, "context-ring");
        assert!(drawn(cx, "context-compact-control"));
        assert!(drawn(cx, "context-auto-compaction"));
        click(cx, "context-ring");
    }
}

struct SizedSession {
    session: Entity<SessionView>,
    width: f32,
}

impl gpui_kit::Render for SizedSession {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div()
            .w(gpui_kit::px(self.width))
            .h_full()
            .child(self.session.clone())
    }
}

#[gpui::test]
fn context_ring_fits_and_opens_in_every_style_and_narrow_pane(cx: &mut TestAppContext) {
    use gentle_core::input_style::InputStyle;
    use gpui_kit::AppContext as _;

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (root, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            view.usage.context_test_ready = true;
            view.usage.context_stats_received = true;
            view.usage.cost = 0.13;
            view
        });
        SizedSession {
            session,
            width: 700.,
        }
    });
    for style in InputStyle::ALL {
        for width in [700., 320.] {
            cx.update(|_, cx| super::input_style::set_test_style(style, cx));
            root.update(cx, |root, cx| {
                root.width = width;
                root.session.update(cx, |view, cx| {
                    view.pane_width.set(width);
                    cx.notify();
                });
                cx.notify();
            });
            cx.run_until_parked();
            let ring = cx.debug_bounds("context-ring").expect("ring drawn");
            assert!(
                f32::from(ring.left()) >= 0. && f32::from(ring.right()) <= width,
                "{style:?} at {width}: {ring:?}"
            );
            if width == 700. {
                let usage = cx.debug_bounds("context-usage-chip").expect("usage drawn");
                assert!(ring.right() <= usage.left(), "{style:?}: ring beside usage");
            }
            click(cx, "context-ring");
            assert!(drawn(cx, "context-auto-compaction"), "{style:?} at {width}");
            assert!(drawn(cx, "context-compact-control"), "{style:?} at {width}");
            click(cx, "context-ring");
            assert!(!drawn(cx, "context-auto-compaction"));
        }
    }
}

#[gpui::test]
fn a_sent_prompt_can_be_selected(cx: &mut TestAppContext) {
    use gpui_kit::{AppContext as _, MouseButton, base::TextSelection};

    struct FocusedSession {
        session: Entity<SessionView>,
        focus: gpui_kit::FocusHandle,
    }

    impl gpui_kit::Render for FocusedSession {
        fn render(
            &mut self,
            _: &mut gpui_kit::Window,
            _: &mut gpui_kit::Context<Self>,
        ) -> impl gpui_kit::IntoElement {
            use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _};
            gpui_kit::div()
                .id("focus")
                .track_focus(&self.focus)
                .size_full()
                .child(self.session.clone())
        }
    }

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text: "fix the parser".into(),
                    images: Vec::new(),
                });
            view.sync_view(cx);
            view
        });
        let view = cx.new(|_| FocusedSession { session, focus });
        gpui_kit::base::Root::new(view, window, cx)
    });
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("user-text-0")
        .expect("the sent prompt is drawn");
    let (start, end) = (
        gpui_kit::point(bounds.left() + gpui_kit::px(1.), bounds.center().y),
        gpui_kit::point(bounds.right() - gpui_kit::px(1.), bounds.center().y),
    );
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| {
        let _ = window.draw(cx);
        assert_eq!(TextSelection::selected_text(window, cx), "fix the parser\n");
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("sentinel".into()));
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("fix the parser".into())
    );
}

/// Selecting a sent bubble copies the displayed text, without matched delimiters.
#[gpui::test]
fn user_bubble_inline_code_selection_omits_matched_delimiters(cx: &mut TestAppContext) {
    use gpui_kit::{AppContext as _, MouseButton, base::TextSelection};

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text: "fix `café` and `x` then `open".into(),
                    images: Vec::new(),
                });
            view.sync_view(cx);
            view
        });
        Selectable(session)
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("user-text-0").expect("sent prompt");
    let start = gpui_kit::point(bounds.left() + gpui_kit::px(1.), bounds.center().y);
    let end = gpui_kit::point(bounds.right() - gpui_kit::px(1.), bounds.center().y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| {
        let _ = window.draw(cx);
        assert_eq!(
            TextSelection::selected_text(window, cx),
            "fix café and x then `open\n"
        );
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()),
        "fix café and x then `open"
    );
}

/// Authored soft breaks stay visible and selection copies the formatted text.
#[gpui::test]
fn user_bubble_authored_newlines_and_clipboard(cx: &mut TestAppContext) {
    use gpui_kit::{AppContext as _, MouseButton, point, px};

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text: "1- `code`\n2- **bold**\n3- ~~strike~~".into(),
                    images: Vec::new(),
                });
            view.sync_view(cx);
            view
        });
        Selectable(session)
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("user-text-0").expect("sent prompt");
    let start = point(bounds.left() + px(1.), bounds.top() + px(5.));
    let first_end = point(bounds.right() - px(1.), bounds.top() + px(5.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(first_end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(first_end, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    });
    cx.run_until_parked();
    let partial = cx.update(|_, cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()
    });
    assert_eq!(partial, "1- code");

    let end = point(bounds.right() - px(1.), bounds.bottom() - px(5.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    });
    cx.run_until_parked();
    let clipboard = cx.update(|_, cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()
    });
    assert!(
        bounds.size.height > px(40.),
        "three authored lines must occupy three rows"
    );
    assert_eq!(clipboard, "1- code\n2- bold\n3- strike");
}

/// Existing list and fenced-code blocks retain their rendered lines and copy text.
#[gpui::test]
fn user_bubble_list_and_fenced_code_preserved(cx: &mut TestAppContext) {
    use gpui_kit::{AppContext as _, MouseButton, point, px};

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for (source, expected) in [
        ("- one\n- two\n- three", "one\ntwo\nthree"),
        ("```\none\ntwo\nthree\n```", "one\ntwo\nthree"),
    ] {
        let (_, cx) = cx.add_window_view(|window, cx| {
            let session = cx.new(|cx| {
                let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
                view.backend.started = true;
                view.transcript
                    .items
                    .push(gentle_core::transcript::Item::User {
                        text: source.into(),
                        images: Vec::new(),
                    });
                view.sync_view(cx);
                view
            });
            Selectable(session)
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("user-text-0").expect("sent prompt");
        assert!(
            bounds.size.height > px(40.),
            "block must occupy multiple rows: {source:?}"
        );
        let start = point(bounds.left() + px(1.), bounds.top() + px(5.));
        let end = point(bounds.right() - px(1.), bounds.bottom() - px(5.));
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        });
        cx.run_until_parked();
        let clipboard = cx.update(|_, cx| {
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap_or_default()
        });
        assert_eq!(clipboard, expected, "source: {source:?}");
    }
}

fn assert_user_bubble_copy(
    cx: &mut TestAppContext,
    source: &str,
    expected: &str,
    start_inset: Option<f32>,
    end_inset: f32,
) {
    use gpui_kit::{AppContext as _, MouseButton, base::TextSelection, point, px};

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text: source.into(),
                    images: Vec::new(),
                });
            view.sync_view(cx);
            view
        });
        Selectable(session)
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("user-text-0").expect("sent prompt");
    let end = point(bounds.right() - px(end_inset), bounds.bottom() - px(10.));
    let start_inset = start_inset.unwrap_or_else(|| {
        // Find a hit inside the first glyph's leading half rather than assuming
        // the list marker's indentation and glyph metrics in pixels.
        (1..200)
            .find(|x| {
                let start = point(bounds.left() + px(*x as f32), bounds.top() + px(10.));
                cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
                cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
                cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
                cx.update(|window, cx| {
                    let _ = window.draw(cx);
                    TextSelection::selected_text(window, cx).trim_end_matches('\n') == expected
                })
            })
            .expect("selection reached the first glyph") as f32
    });
    let start = point(bounds.left() + px(start_inset), bounds.top() + px(10.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    let selected = cx.update(|window, cx| {
        let _ = window.draw(cx);
        TextSelection::selected_text(window, cx)
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    });
    cx.run_until_parked();
    let clipboard = cx.update(|_, cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()
    });
    assert_eq!(selected.trim_end_matches('\n'), expected, "selection");
    assert_eq!(clipboard, expected, "clipboard");
}

#[gpui::test]
fn user_bubble_complete_html_fragments_are_literal(cx: &mut TestAppContext) {
    for (source, expected) in [
        (
            "<div><span>hola</span></div>",
            "<div><span>hola</span></div>",
        ),
        (
            "**outside** <span title=\"**attr**\">**inside**</span> **after**",
            "outside <span title=\"**attr**\">**inside**</span> after",
        ),
        (
            "<div><br><span title=\"a > b\">**hola**</span></div>",
            "<div><br><span title=\"a > b\">**hola**</span></div>",
        ),
    ] {
        assert_user_bubble_copy(cx, source, expected, Some(1.), 1.);
    }
}

#[gpui::test]
fn user_bubble_html_comment_does_not_close_fragment(cx: &mut TestAppContext) {
    let source = "<div><!-- </div> -->**inside**</div>";
    assert_user_bubble_copy(cx, source, source, Some(1.), 1.);
}

#[gpui::test]
fn user_bubble_folded_html_keeps_content_literal(cx: &mut TestAppContext) {
    let source = "<div>\n**one**\n**two**\n**three**\n**four**\n**five**\n**six**\n**seven**\n**eight**\n**nine**\n</div>";
    let expected = "<div>\n**one**\n**two**\n**three**\n**four**\n**five**\n**six**\n**seven**";
    assert_user_bubble_copy(cx, source, expected, Some(1.), 1.);
}

#[gpui::test]
fn user_bubble_html_indented_content_has_no_escape_backslashes(cx: &mut TestAppContext) {
    let source = "<div>\n\n    **literal**\n</div>";
    assert_user_bubble_copy(cx, source, source, Some(1.), 1.);
}

#[gpui::test]
fn user_bubble_html_tab_indentation_is_literal(cx: &mut TestAppContext) {
    let source = "<div>\n\n\t**literal**\n</div>";
    assert_user_bubble_copy(cx, source, source, Some(1.), 1.);
}

#[gpui::test]
fn user_bubble_raw_html_is_visible(cx: &mut TestAppContext) {
    assert_user_bubble_copy(
        cx,
        "<span title=\"**important**\">text</span>",
        "<span title=\"**important**\">text</span>",
        Some(1.),
        1.,
    );
    assert_user_bubble_copy(
        cx,
        "hello <div>world</div>",
        "hello <div>world</div>",
        Some(1.),
        1.,
    );
    assert_user_bubble_copy(
        cx,
        "<div>\nhello\n</div>",
        "<div>\nhello\n</div>",
        Some(1.),
        1.,
    );
    assert_user_bubble_copy(
        cx,
        "**bold** `<div>` [link](https://example.com) &amp;",
        "bold <div> link &",
        Some(1.),
        1.,
    );
}

#[gpui::test]
fn user_bubble_container_fenced_code_copy(cx: &mut TestAppContext) {
    assert_user_bubble_copy(
        cx,
        "- item\n\n  ```\n  one\n  two\n  ```",
        "item\none\ntwo",
        None,
        8.,
    );
}

#[gpui::test]
fn user_bubble_triple_backtick_inline_copy(cx: &mut TestAppContext) {
    assert_user_bubble_copy(
        cx,
        "```code```\nnext\nlast",
        "code\nnext\nlast",
        Some(1.),
        8.,
    );
}

#[gpui::test]
fn user_bubble_backslash_hard_break_copy(cx: &mut TestAppContext) {
    assert_user_bubble_copy(cx, "one\\\ntwo", "one\ntwo", Some(1.), 1.);
}

/// S7: a message sent before images left the composer still carries
/// `[Image #N]`; its bubble shows the chip, as it did then.
#[gpui::test]
fn user_bubble_shows_old_image_tokens_as_chips(cx: &mut TestAppContext) {
    assert_user_bubble_copy(cx, "see [Image #1]", "see 🖼 #1", None, 1.);
}

/// The transcript items of a view showing `messages` as sent prompts, drawn
/// in a headless window.
fn prompts<'a>(messages: &[&str], cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
    use gpui_kit::AppContext as _;

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let messages: Vec<String> = messages.iter().map(|m| m.to_string()).collect();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            for text in messages {
                view.transcript
                    .items
                    .push(gentle_core::transcript::Item::User {
                        text,
                        images: Vec::new(),
                    });
            }
            view.sync_view(cx);
            view
        });
        Selectable(session)
    });
    cx.run_until_parked();
    cx
}

/// S9: a sent prompt's files show as tiles beside the images, and never as
/// the `Attached files:` block in the text: the bubble reads as what was typed
/// (and copies as it), so a long block adds no lines.
#[gpui::test]
fn sent_files_show_as_tiles_and_stay_out_of_the_text(cx: &mut TestAppContext) {
    use gpui_kit::px;

    let message =
        "Qué te envio?\n\nAttached files:\n- /Users/me/Comu 3/Laboratorio 1.6.2.pdf\n- notes.txt";
    assert_user_bubble_copy(cx, message, "Qué te envio?", Some(1.), 1.);

    let cx = prompts(&[message], cx);
    let bounds = cx.debug_bounds("user-text-0").expect("the sent prompt");
    assert!(
        bounds.size.height < px(40.),
        "the text is one line, not the block's three: {}",
        bounds.size.height
    );
    assert!(cx.debug_bounds("user-file-0-0").is_some(), "first card");
    assert!(cx.debug_bounds("user-file-0-1").is_some(), "second card");
    assert!(
        cx.debug_bounds("user-file-0-2").is_none(),
        "one card per file"
    );
}

/// S9: a prompt that was only files shows their cards and no empty text
/// block, as an image-only one does.
#[gpui::test]
fn a_files_only_prompt_shows_cards_without_an_empty_text(cx: &mut TestAppContext) {
    let cx = prompts(&["Attached files:\n- report.pdf"], cx);
    assert!(cx.debug_bounds("user-file-0-0").is_some(), "the card");
    assert!(
        cx.debug_bounds("user-text-0").is_none(),
        "no empty text block"
    );
}

/// S13: a strip is even. In a sent bubble an image and two files (one with a
/// folder, one at the root) are tiles of the same size, on the image's row.
#[gpui::test]
fn a_sent_strip_is_one_row_of_equal_tiles(cx: &mut TestAppContext) {
    use gpui_kit::AppContext as _;

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let text = "Mira\n\nAttached files:\n- ~/Documents/documento.pdf\n- test.txt";
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            let mut view = SessionView::new(std::env::temp_dir(), None, window, cx, false);
            view.backend.started = true;
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text: text.to_string(),
                    images: vec![gentle_core::transcript::UserImage::new(
                        "image/png",
                        "iVBORw0KGgo=".into(),
                    )],
                });
            view.sync_view(cx);
            view
        });
        Selectable(session)
    });
    cx.run_until_parked();

    let image = bounds(cx, "user-image-0-0");
    let with_folder = bounds(cx, "user-file-0-0");
    let at_root = bounds(cx, "user-file-0-1");
    assert_eq!(
        image.size, with_folder.size,
        "a file tile is the image tile's size"
    );
    assert_eq!(image.size, at_root.size, "both files, the same tile");
    assert_eq!(
        (image.origin.y, image.bottom()),
        (with_folder.origin.y, with_folder.bottom()),
        "the files sit on the image's row"
    );
    assert_eq!(
        (image.origin.y, image.bottom()),
        (at_root.origin.y, at_root.bottom())
    );
}

/// S13: the composer's strip is even the same way: an attached image and two
/// files draw tiles of one size, on one row.
#[gpui::test]
fn the_composer_strip_is_one_row_of_equal_tiles(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let image = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
    let file = |name: &str, path: &str, bytes: u64| {
        Ok(Loaded::File(AttachedFile {
            name: name.to_string(),
            path: std::path::PathBuf::from(path),
            canonical: std::path::PathBuf::from(path),
            bytes,
        }))
    };
    view.update_in(cx, |view, window, cx| {
        view.attach(
            vec![
                Ok(Loaded::Image(image)),
                file("documento.pdf", "/Users/me/Documents/documento.pdf", 2048),
                file("test.txt", "/Users/me/test.txt", 12),
            ],
            window,
            cx,
        );
    });

    let image = bounds(cx, "composer-image-0");
    let with_folder = bounds(cx, "composer-file-0");
    let at_root = bounds(cx, "composer-file-1");
    assert_eq!(
        image.size, with_folder.size,
        "a file tile is the image tile's size"
    );
    assert_eq!(image.size, at_root.size, "both files, the same tile");
    assert_eq!(
        (image.origin.y, image.bottom()),
        (with_folder.origin.y, with_folder.bottom()),
        "the files sit on the image's row"
    );
    assert_eq!(
        (image.origin.y, image.bottom()),
        (at_root.origin.y, at_root.bottom())
    );
}

/// S13: a file tile keeps the × its card had, in the same corner an image's ×
/// sits in, and it removes that file.
#[gpui::test]
fn a_composer_file_tile_keeps_its_remove_button(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let image = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
    let file = |name: &str, path: &str| {
        Ok(Loaded::File(AttachedFile {
            name: name.to_string(),
            path: std::path::PathBuf::from(path),
            canonical: std::path::PathBuf::from(path),
            bytes: 12,
        }))
    };
    view.update_in(cx, |view, window, cx| {
        view.attach(
            vec![
                Ok(Loaded::Image(image)),
                file("documento.pdf", "/work/documento.pdf"),
                file("test.txt", "/work/test.txt"),
            ],
            window,
            cx,
        );
    });

    let corner = |tile: gpui_kit::Bounds<gpui_kit::Pixels>,
                  remove: gpui_kit::Bounds<gpui_kit::Pixels>| {
        (remove.left() - tile.left(), remove.top() - tile.top())
    };
    let on_image = bounds(cx, "attachment-remove-0");
    let image_tile = bounds(cx, "composer-image-0");
    let on_file = bounds(cx, "file-remove-0");
    let file_tile = bounds(cx, "composer-file-0");
    assert_eq!(
        corner(image_tile, on_image),
        corner(file_tile, on_file),
        "a file's × sits where an image's does"
    );
    assert!(
        file_tile.contains(&on_file.center()),
        "the × is inside the tile"
    );

    click(cx, "file-remove-0");
    assert_eq!(
        files(&view, cx),
        vec![std::path::PathBuf::from("/work/test.txt")],
        "the × removes its own file"
    );
}

/// Attaching an image types nothing into the composer, so the strip above it
/// is the only list of what will be sent; removing one from the strip leaves
/// the rest, in the strip's own order.
#[gpui::test]
fn images_attach_without_text_and_the_strip_is_what_is_sent(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let first = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
    let second = Attachment::from_bytes("b.jpeg", JPEG.to_vec()).unwrap();
    let second_key = second.key;
    view.update_in(cx, |view, window, cx| {
        view.attach(
            vec![Ok(Loaded::Image(first)), Ok(Loaded::Image(second))],
            window,
            cx,
        );
    });
    let text = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string())
    };
    assert_eq!(text(cx), "", "attaching an image types nothing");
    assert_eq!(view.read_with(cx, |view, _| view.attachments.len()), 2);

    view.update_in(cx, |view, _, cx| view.remove_attachment(0, cx));
    assert_eq!(text(cx), "", "removing one types nothing either");
    let sent: Vec<u64> = view.read_with(cx, |view, _| {
        view.attachments.iter().map(|a| a.key).collect()
    });
    assert_eq!(sent, vec![second_key], "the strip's images, in its order");
}

/// S1: a copied file that is not an image pastes as a file attachment, like
/// a dropped one, and types nothing into the composer.
#[gpui::test]
fn pasting_a_copied_file_attaches_it(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let dir = std::env::temp_dir().join(format!("gs-paste-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();
    let item = gpui_kit::ClipboardItem {
        entries: vec![gpui_kit::ClipboardEntry::ExternalPaths(
            gpui_kit::ExternalPaths(vec![report.clone()].into()),
        )],
    };

    let handled = view.update_in(cx, |view, window, cx| {
        view.paste_clipboard(&item, window, cx)
    });
    cx.run_until_parked();

    assert!(handled, "the paste is taken as an attachment");
    let files: Vec<_> = view.read_with(cx, |view, _| {
        view.files.iter().map(|file| file.path.clone()).collect()
    });
    assert_eq!(files, vec![report]);
    assert_eq!(
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string()),
        "",
        "pasting a file types nothing"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// S11: the prompt names files as the tab's backend reads them: a WSL tab
/// maps both the file and the folder (so one inside the folder stays
/// relative and one outside it is that backend's path), while a tab with no
/// backend yet keeps this computer's spelling.
#[gpui::test]
fn a_prompt_names_its_files_as_the_tabs_backend_reads_them(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let attached = |name: &str, path: &str| AttachedFile {
        name: name.to_string(),
        path: std::path::PathBuf::from(path),
        canonical: std::path::PathBuf::from(path),
        bytes: 0,
    };
    view.update(cx, |view, _| view.cwd = std::path::PathBuf::from("/work"));
    let files = [
        attached("main.rs", "/work/src/main.rs"),
        attached("a.pdf", "/media/a.pdf"),
    ];
    assert_eq!(
        view.read_with(cx, |view, _| view.prompt_message("look", &files)),
        "look\n\nAttached files:\n- src/main.rs\n- /media/a.pdf"
    );

    view.update(cx, |view, _| {
        view.cwd = std::path::PathBuf::from(r"C:\proj");
        view.runs_on = Some(gentle_core::backend::Backend::Wsl {
            distro: "Ubuntu".to_string(),
        });
    });
    let files = [
        attached("main.rs", r"C:\proj\src\main.rs"),
        attached("a.pdf", r"C:\Users\x\Downloads\a.pdf"),
    ];
    assert_eq!(
        view.read_with(cx, |view, _| view.prompt_message("look", &files)),
        "look\n\nAttached files:\n- src/main.rs\n- /mnt/c/Users/x/Downloads/a.pdf"
    );
}

/// S11, S14/R4: attaching the same file twice keeps one card, whatever route it came
/// by (its path for a file, its bytes for an image), and removing the card
/// lets it be attached again.
#[gpui::test]
fn attaching_the_same_file_twice_keeps_one_card(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let dir = std::env::temp_dir().join(format!("gs-attach-twice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();
    let shot = dir.join("shot.png");
    std::fs::write(&shot, PNG).unwrap();

    view.update_in(cx, |view, window, cx| {
        view.attach_paths(
            vec![report.clone(), report.clone(), shot.clone(), shot.clone()],
            window,
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.files.len()), 1);
    assert_eq!(view.read_with(cx, |view, _| view.attachments.len()), 1);

    view.update_in(cx, |view, window, cx| {
        view.remove_file(0, cx);
        view.attach_paths(vec![report.clone()], window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.files.len()),
        1,
        "removing the card lets the file be attached again"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// S14/R4: one file reached by another spelling is one tile — `./` in its
/// path, a symlink to it — and the tile, the tooltip and the prompt still
/// name the path as it was attached.
#[gpui::test]
fn another_spelling_of_one_file_keeps_one_tile(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let dir = std::env::temp_dir().join(format!("gs-attach-spelled-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();
    let dotted = dir.join("./report.pdf");

    view.update_in(cx, |view, window, cx| {
        view.attach_paths(vec![dotted.clone(), report.clone()], window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        files(&view, cx),
        vec![dotted.clone()],
        "`./report.pdf` and `report.pdf` are one tile, named as it was attached"
    );

    view.update_in(cx, |view, _, cx| view.remove_file(0, cx));
    #[cfg(unix)]
    {
        let link = dir.join("report-link.pdf");
        std::os::unix::fs::symlink(&report, &link).unwrap();
        view.update_in(cx, |view, window, cx| {
            view.attach_paths(vec![report.clone(), link.clone()], window, cx);
        });
        cx.run_until_parked();
        assert_eq!(
            files(&view, cx),
            vec![report],
            "a symlink to an attached file is the same file"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// S11, S14/R3-2: a file gone from disk when the prompt would be sent stops
/// the send on the path Enter takes, and names it; the text and the tile stay
/// so the user can remove it. A second Enter while the check is in flight is
/// dropped, so one send is stopped once.
#[gpui::test]
fn a_file_gone_before_sending_stops_the_send(cx: &mut TestAppContext) {
    let (view, cx) = rooted_tab(cx);
    let dir = std::env::temp_dir().join(format!("gs-attach-gone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();

    view.update_in(cx, |view, window, cx| {
        view.attach_paths(vec![report.clone()], window, cx);
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.files.len()), 1);

    // The file is there: Enter finds nothing to refuse and clears nothing,
    // since this tab never started a backend to send to.
    type_text(&view, cx, "send the report");
    press_enter(&view, cx);
    cx.run_until_parked();
    assert_eq!(warnings(cx), 0, "a file that is there stops nothing");
    assert_eq!(
        composer_text(&view, cx),
        "send the report",
        "with no backend the text stays where it was"
    );
    assert_eq!(view.read_with(cx, |view, _| view.files.len()), 1);

    std::fs::remove_file(&report).unwrap();
    press_enter(&view, cx);
    // A second Enter arrives while the check is in flight: it starts no send.
    press_enter(&view, cx);
    cx.run_until_parked();
    assert_eq!(
        warnings(cx),
        1,
        "the send is stopped once, with a warning naming the file"
    );
    assert_eq!(
        composer_text(&view, cx),
        "send the report",
        "nothing was cleared: the text is still there to send"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.files.len()),
        1,
        "the tile stays, for the user to remove it"
    );
    assert_eq!(files(&view, cx), vec![report], "and it is still that file");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// T12/S14: the press that asked the disk is the press that may still send.
/// Removing the file's tile while the check runs ends that press: nothing is
/// sent, cleared or remembered, and no warning explains a change the user
/// made (Enter again starts the check over).
#[gpui::test]
fn a_composer_changed_while_the_files_are_checked_sends_nothing(cx: &mut TestAppContext) {
    let (view, cx) = rooted_tab(cx);
    let dir = std::env::temp_dir().join(format!("gs-attach-changed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();

    view.update_in(cx, |view, window, cx| {
        view.attach_paths(vec![report], window, cx);
    });
    cx.run_until_parked();
    assert_eq!(files(&view, cx).len(), 1);

    type_text(&view, cx, "send the report");
    let history = view.read_with(cx, |view, _| view.prompt_history.clone());
    press_enter(&view, cx);
    // While the disk is asked, the tile goes.
    view.update(cx, |view, cx| view.remove_file(0, cx));
    cx.run_until_parked();

    assert_eq!(
        warnings(cx),
        0,
        "the user changed it: nothing to warn about"
    );
    assert_eq!(
        composer_text(&view, cx),
        "send the report",
        "nothing was cleared"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.prompt_history.clone()),
        history,
        "nothing was remembered"
    );
    assert!(
        files(&view, cx).is_empty(),
        "the tile the user removed stays removed"
    );
    assert!(
        !view.read_with(cx, |view, _| view.checking_files),
        "and the next Enter may ask the disk again"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// ↑ recalls a sent prompt while an image waits: only the text changes, so
/// the strip still sends the image it shows.
#[gpui::test]
fn history_recall_leaves_a_waiting_image_in_the_strip(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let image = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
    let key = image.key;
    view.update(cx, |view, _| view.prompt_history.record("first"));
    view.update_in(cx, |view, window, cx| {
        view.attach(vec![Ok(Loaded::Image(image))], window, cx);
    });
    cx.update(|window, cx| {
        let composer = view.read(cx).composer.clone();
        composer.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_value("draft", window, cx);
        });
    });
    cx.simulate_keystrokes("up");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string()),
        "first"
    );
    let sent: Vec<u64> = view.read_with(cx, |view, _| {
        view.attachments.iter().map(|a| a.key).collect()
    });
    assert_eq!(sent, vec![key]);
}

/// S9: the ↑ history of a resumed conversation holds what was typed, never
/// the `Attached files:` block a prompt carried; a prompt that was only
/// files is not recalled at all.
#[gpui::test]
fn history_recall_leaves_the_file_block_out(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, _| {
        for text in [
            "look\n\nAttached files:\n- a.pdf",
            "Attached files:\n- b.txt",
            "plain",
        ] {
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text: text.into(),
                    images: Vec::new(),
                });
        }
        view.seed_prompt_history();
    });
    cx.update(|window, cx| {
        let composer = view.read(cx).composer.clone();
        composer.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_value("draft", window, cx);
        });
    });
    let composer = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string())
    };
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "plain");
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "look", "the block is not recalled");
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "look", "a files-only prompt is not recalled");
}

/// A skill echo with a body that contains its own closing tag.
const ECHO: &str = "<skill name=\"docs\" location=\"/x\">\n```xml\n</skill>\n```\n</skill>\n\nlook";

fn push_user(view: &mut SessionView, text: &str) {
    view.transcript.apply(&serde_json::json!({
        "type": "message_start",
        "message": {"role": "user", "content": text},
    }));
}

/// S1/S3 (R3-001): the echo confirms only when its prompt was sent idle and
/// the pending text is the command that produced it.
#[gpui::test]
fn live_skill_echo_matches_only_the_idle_pending_command(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        for (ix, (original, idle)) in [
            (ECHO, true),
            ("/skill:docs look", false),
            ("/skill:docs look", true),
        ]
        .into_iter()
        .enumerate()
        {
            view.live.expect_reply(original, Vec::new(), ix, idle);
            push_user(view, ECHO);
            view.sync_view(cx);
        }
        assert!(!view.live.skills.contains_key(&0));
        assert!(!view.live.skills.contains_key(&1));
        assert_eq!(view.live.skills[&2].original, "/skill:docs look");
    });
}

/// S3 (R3-001): the real send path registers a pending reply as it did before
/// skills collapsed (not while steering), and marks only an idle send as one
/// whose echo may be confirmed.
#[gpui::test]
fn send_registers_the_pending_reply_unless_steering(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.transcript.running = true;
        view.send_prompt_with("steer".into(), Vec::new(), cx);
        assert!(
            view.live.pending.is_none(),
            "a steered prompt waits for nothing"
        );

        view.transcript.running = false;
        view.send_prompt_with("one".into(), Vec::new(), cx);
        assert!(
            view.live
                .pending
                .as_ref()
                .is_some_and(|p| p.idle && p.text == "one")
        );

        // Another prompt while the first is not picked up: still tracked as
        // before, but its echo cannot be told apart.
        view.send_prompt_with("/skill:docs look".into(), Vec::new(), cx);
        assert!(
            view.live
                .pending
                .as_ref()
                .is_some_and(|p| !p.idle && p.text == "/skill:docs look")
        );
    });
}

/// S3 (R3-002): a confirmation never renders or copies for another item at
/// its index. A transcript rebuilt by a refresh drops it, so the same text at
/// the same index renders and copies as ordinary text.
#[gpui::test]
fn a_confirmation_is_only_for_its_own_echo(cx: &mut TestAppContext) {
    let (view, cx) = rooted_tab(cx);
    view.update(cx, |view, cx| {
        view.live
            .expect_reply("/skill:docs look", Vec::new(), 0, true);
        push_user(view, ECHO);
        view.sync_view(cx);
        assert!(view.live.confirmed(0, &view.transcript.items[0]).is_some());

        // Truncated and refilled without a reset: another message at 0.
        view.transcript.items.clear();
        push_user(view, "something else");
        assert!(view.live.confirmed(0, &view.transcript.items[0]).is_none());
    });

    // The real reload path: `get_messages` replaced the transcript with one
    // user message at index 0, which is `ECHO` again. The rebuild drops the
    // stale confirmation, so the item is no longer treated as a skill
    // invocation.
    view.update(cx, |view, cx| {
        view.apply_messages(&[serde_json::json!(
            {"role": "user", "content": ECHO}
        )]);
        view.sync_view(cx);
        assert!(
            view.live.skills.is_empty(),
            "the stale confirmation is dropped"
        );
        assert!(
            view.live.confirmed(0, &view.transcript.items[0]).is_none(),
            "the rebuilt item is ordinary text, not a skill invocation"
        );
        cx.notify();
    });
    cx.run_until_parked();

    // And the rebuilt item renders and copies as the ordinary text it is.
    assert!(drawn(cx, "user-text-0"));
    assert!(!drawn(cx, "skill-body-0"));
    let bounds = cx.debug_bounds("user-text-0").expect("the typed text");
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
    cx.run_until_parked();
    click(cx, "copy-message-0");
    assert_eq!(
        cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some(ECHO.to_string()),
        "Copy reads the ordinary text, not a skill command"
    );
}

/// S3 (R3-003): reloading the transcript clears confirmations but leaves an
/// outstanding reply alone; an empty reload changes nothing.
#[gpui::test]
fn a_refresh_keeps_the_reply_in_flight(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.live
            .expect_reply("/skill:docs look", Vec::new(), 0, true);
        push_user(view, ECHO);
        view.sync_view(cx);
        assert_eq!(view.live.skills.len(), 1);

        view.apply_messages(&[]);
        view.sync_view(cx);
        assert_eq!(
            view.live.skills.len(),
            1,
            "an empty refresh changes nothing"
        );
        assert_eq!(view.transcript.items.len(), 1);

        view.live.expect_reply("later prompt", Vec::new(), 1, true);
        view.apply_messages(&[serde_json::json!({"role": "user", "content": "other"})]);
        assert!(
            view.live.pending.is_some(),
            "the refresh does not cancel the reply"
        );
        view.sync_view(cx);
        assert!(view.live.skills.is_empty());
    });
}

/// S3 (R3-004): a confirmed skill that carried files keeps its collapsed
/// weight when it is re-derived from index 0 (not by a reset), as a shell
/// finishing before it would.
#[gpui::test]
fn a_confirmed_skill_with_files_keeps_its_weight(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let big = "line\n".repeat(30_000);
    let echo = format!(
        "<skill name=\"docs\" location=\"/x\">\n{big}\n</skill>\n\nlook\n\nAttached files:\n- a.pdf"
    );
    view.update(cx, |view, cx| {
        // A running shell item before the confirmed row: finishing it (as a
        // real shell reply does) dirties from index 0, re-deriving the
        // confirmed item at index 1 without resetting the transcript.
        let shell = view.transcript.start_shell("!build".into());
        view.live.expect_reply(
            "/skill:docs look\n\nAttached files:\n- a.pdf",
            Vec::new(),
            1,
            true,
        );
        push_user(view, &echo);
        view.sync_view(cx);
        let first = view.list.weight(1).unwrap();
        assert!(first < 20., "{first}");
        view.live.reply_failed();
        view.transcript.finish_shell(shell, Some(0), None);
        view.sync_view(cx);
        assert_eq!(view.list.weight(1), Some(first));
    });
}

/// S3 (R3-006): Copy and the skill row, clicked on a confirmed live
/// invocation, copy the command and expand the body.
#[gpui::test]
fn a_confirmed_skill_row_copies_its_command_and_expands(cx: &mut TestAppContext) {
    let (view, cx) = rooted_tab(cx);
    view.update(cx, |view, cx| {
        view.live.expect_reply(
            "/skill:docs look\n\nAttached files:\n- a.pdf",
            Vec::new(),
            0,
            true,
        );
        push_user(view, &format!("{ECHO}\n\nAttached files:\n- a.pdf"));
        view.sync_view(cx);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(!drawn(cx, "skill-body-0"));
    click(cx, "fold:docs");
    assert!(drawn(cx, "skill-body-0"), "the skill row expands the body");
    let bounds = cx.debug_bounds("user-text-0").expect("the typed args");
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
    click(cx, "copy-message-0");
    assert_eq!(
        cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("/skill:docs look".into())
    );
}

/// S1: a confirmed live skill row weighs by its visible content, not its
/// hidden body; the same wrapper unconfirmed weighs as ordinary text.
#[gpui::test]
fn confirmed_skill_rows_weigh_independent_of_body(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let echo =
        |body: &str| format!("<skill name=\"docs\" location=\"/x\">\n{body}\n</skill>\n\nlook");
    view.update(cx, |view, cx| {
        let big = "line\n".repeat(30_000);
        let rows = [
            (echo("short"), true),
            (echo(&big), true),
            (echo(&big), false),
        ];
        for (ix, (text, confirmed)) in rows.into_iter().enumerate() {
            if confirmed {
                view.live
                    .expect_reply("/skill:docs look", Vec::new(), ix, true);
            }
            view.transcript.apply(&serde_json::json!({
                "type": "message_start",
                "message": {"role": "user", "content": text},
            }));
            view.sync_view(cx);
            view.live.reply_failed();
        }
        let weight = |ix| view.list.weight(ix).unwrap();
        assert_eq!(weight(0), weight(1));
        assert!(weight(2) > weight(1) * 2.);
        // Re-deriving a later item keeps the earlier confirmation.
        view.transcript.apply(&serde_json::json!({
            "type": "message_start",
            "message": {"role": "user", "content": "x"},
        }));
        view.sync_view(cx);
        assert_eq!(view.list.weight(1), view.list.weight(0));
    });
}

/// S2: a resumed wrapper is ordinary text in history.
#[gpui::test]
fn history_recall_keeps_resumed_skill_text(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let block = "<skill name=\"docs\" location=\"/x/SKILL.md\">\nLoaded body.\n</skill>";
    view.update(cx, |view, _| {
        for text in [format!("{block}\n\nsummarize"), block.to_string()] {
            view.transcript
                .items
                .push(gentle_core::transcript::Item::User {
                    text,
                    images: Vec::new(),
                });
        }
        view.seed_prompt_history();
    });
    cx.update(|window, cx| {
        let composer = view.read(cx).composer.clone();
        composer.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_value("draft", window, cx);
        });
    });
    let composer = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string())
    };
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), block);
    cx.simulate_keystrokes("up up up"); // Cross the three-line wrapper, then recall the older entry.
    assert_eq!(composer(cx), format!("{block}\n\nsummarize"));
}

/// ↑ and ↓ in the composer walk the prompts sent before and come back to
/// the draft; inside a multi-line prompt ↑ moves the cursor first.
#[gpui::test]
fn arrows_recall_sent_prompts_and_return_to_the_draft(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, _| {
        view.prompt_history.record("first");
        view.prompt_history.record("second\nline two");
    });
    cx.update(|window, cx| {
        let composer = view.read(cx).composer.clone();
        composer.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_value("draft", window, cx);
        });
    });
    let composer = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string())
    };
    assert_eq!(composer(cx), "draft");

    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "second\nline two");
    // The cursor sits at the end, on the second line: ↑ moves it up first.
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "second\nline two");
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "first");
    cx.simulate_keystrokes("up");
    assert_eq!(composer(cx), "first", "nothing older than the oldest");
    cx.simulate_keystrokes("down");
    assert_eq!(composer(cx), "second\nline two");
    cx.simulate_keystrokes("down");
    assert_eq!(composer(cx), "draft");
}

/// Esc closes what is open first; with nothing open it stops the running
/// turn, unless it comes right after an Esc that closed something.
#[gpui::test]
fn escape_closes_first_then_stops_the_turn(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let escape = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.abort(&super::Abort, window, cx));
        });
    };
    let stopping = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.live.stopping);
    view.update(cx, |view, cx| {
        view.transcript.running = true;
        view.sync_view(cx);
        view.agents_panel.open = true;
    });
    assert!(view.read_with(cx, |view, _| view.turn_since().is_some()));

    escape(cx);
    assert!(view.read_with(cx, |view, _| !view.agents_panel.open));
    assert_eq!(stopping(cx), None, "the first Esc only closed the panel");
    escape(cx);
    assert_eq!(stopping(cx), None, "a quick second Esc doesn't stop");

    view.update(cx, |view, _| view.live.escape_closed = None);
    escape(cx);
    let turn = view.read_with(cx, |view, _| view.turn_since());
    assert_eq!(stopping(cx), turn, "with nothing open, Esc stops the turn");
}

/// Counts the tab's requests to close its pane; they count while the
/// subscription lives.
fn close_requests(
    view: &Entity<SessionView>,
    cx: &mut VisualTestContext,
) -> (Rc<RefCell<usize>>, Subscription) {
    let count: Rc<RefCell<usize>> = Rc::default();
    let seen = count.clone();
    let subscription = cx.cx.update(|cx| {
        cx.subscribe(view, move |_, event: &super::SessionEvent, _| {
            if matches!(event, super::SessionEvent::CloseRequested) {
                *seen.borrow_mut() += 1;
            }
        })
    });
    (count, subscription)
}

/// Lets go of Ctrl+C: the next press is a new one, not a key repeat.
fn release_ctrl_c(cx: &mut VisualTestContext) {
    cx.simulate_event(gpui_kit::KeyUpEvent {
        keystroke: gpui_kit::Keystroke::parse("ctrl-c").unwrap(),
    });
}

/// Two Ctrl+C on the idle agent ask the window to close its pane, sending
/// nothing and leaving the draft alone; a held key or any other key between
/// the presses starts over.
#[gpui::test]
fn double_ctrl_c_on_the_idle_agent_closes_its_pane(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let (closes, _subscription) = close_requests(&view, cx);
    let armed = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.live.double_interrupt.armed().is_some())
    };
    type_in_composer(&view, cx, "exit");

    cx.simulate_keystrokes("ctrl-c");
    assert!(armed(cx), "the first press shows the hint");
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(*closes.borrow(), 0, "a key repeat never closes");
    assert!(!armed(cx), "and drops the first press");
    release_ctrl_c(cx);

    cx.simulate_keystrokes("ctrl-c");
    release_ctrl_c(cx);
    cx.simulate_keystrokes("left");
    assert!(!armed(cx), "another key starts over");
    cx.simulate_keystrokes("ctrl-c");
    assert!(armed(cx));
    release_ctrl_c(cx);
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(*closes.borrow(), 1, "the second press closes");
    assert!(!armed(cx));
    assert_eq!(composer_text(&view, cx), "exit");
    assert_eq!(sent_prompts(&view, cx), 0);
}

/// Ctrl+C while a turn runs stops it, as Esc does, and never arms the close:
/// the next press, the agent idle again, only shows the hint.
#[gpui::test]
fn ctrl_c_while_a_turn_runs_only_stops_it(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let (closes, _subscription) = close_requests(&view, cx);
    type_in_composer(&view, cx, "");
    view.update(cx, |view, cx| {
        view.transcript.running = true;
        view.sync_view(cx);
    });

    cx.simulate_keystrokes("ctrl-c");
    let (stopping, turn, armed) = view.read_with(cx, |view, _| {
        (
            view.live.stopping,
            view.turn_since(),
            view.live.double_interrupt.armed(),
        )
    });
    assert!(turn.is_some());
    assert_eq!(stopping, turn, "the press stops the turn");
    assert_eq!(armed, None, "and doesn't arm the close");

    release_ctrl_c(cx);
    view.update(cx, |view, cx| {
        view.transcript.running = false;
        view.sync_view(cx);
    });
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(*closes.borrow(), 0);
    assert!(view.read_with(cx, |view, _| view.live.double_interrupt.armed().is_some()));
}

/// Off macOS Ctrl+C is also Copy: with text selected it copies and isn't a
/// press. On macOS (⌘C copies) it is a press all the same.
#[gpui::test]
fn ctrl_c_with_text_selected_copies_off_macos(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    type_in_composer(&view, cx, "exit");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    cx.update(|_, cx| {
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("sentinel".into()))
    });

    cx.simulate_keystrokes("ctrl-c");
    let armed = view.read_with(cx, |view, _| view.live.double_interrupt.armed().is_some());
    let clipboard = cx.update(|_, cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()
    });
    if cfg!(target_os = "macos") {
        assert!(armed);
        assert_eq!(clipboard, "sentinel");
    } else {
        assert!(!armed, "copying isn't a press");
        assert_eq!(clipboard, "exit");
    }
}

/// Moving an agent to another folder asks first only when that would lose
/// something: what was said, a running turn or a conversation about to resume.
#[gpui::test]
fn has_conversation_counts_turns_and_pending_resumes(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let has = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.has_conversation());
    assert!(!has(cx), "a fresh tab has nothing to lose");

    view.update(cx, |view, _| view.transcript.running = true);
    assert!(has(cx), "a running turn counts");

    view.update(cx, |view, _| {
        view.transcript.running = false;
        view.backend.pending_session = Some("saved.jsonl".into());
    });
    assert!(has(cx), "a conversation waiting to be resumed counts");
}

/// Changing folder starts fresh: a conversation waiting to be resumed is
/// dropped instead of resuming in the new folder.
#[gpui::test]
fn change_cwd_drops_a_pending_resume(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    // Not started yet: the next start would resume it (`--session`).
    view.update(cx, |view, _| {
        view.backend.started = false;
        view.backend.pending_session = Some("saved.jsonl".into());
    });
    let folder = std::env::temp_dir().join("elsewhere");
    view.update_in(cx, |view, window, cx| {
        view.change_cwd(folder.clone(), window, cx)
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.cwd(), &folder);
        assert!(view.backend.pending_session.is_none());
        assert!(!view.has_conversation());
    });
}

// ── The drop target (S10) ──────────────────────────────────────────────────

/// The drawn bounds of `selector`, after a frame.
fn bounds(
    cx: &mut VisualTestContext,
    selector: &'static str,
) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    cx.run_until_parked();
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("nothing drawn as {selector}"))
}

/// A tab's attached file paths.
fn files(view: &Entity<SessionView>, cx: &mut VisualTestContext) -> Vec<std::path::PathBuf> {
    view.read_with(cx, |view, _| {
        view.files.iter().map(|file| file.path.clone()).collect()
    })
}

/// S10: files dragged over the tab veil it — one layer over the conversation
/// and the composer, icon and words in the middle — the drop attaches them and
/// takes the veil down, and a drag that leaves the tab (or ends without
/// dropping) leaves no veil behind. Nothing about the veil stops the drop.
#[gpui::test]
fn a_file_drag_veils_the_tab_and_the_drop_attaches(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let dir = std::env::temp_dir().join(format!("gs-drop-veil-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();
    let dragged = || gpui_kit::ExternalPaths(vec![report.clone()].into());

    let pane = bounds(cx, super::view::DROP_TARGET);
    cx.simulate_event(FileDropEvent::Entered {
        position: pane.center(),
        paths: dragged(),
    });
    let veil = bounds(cx, super::view::DROP_VEIL);
    assert_eq!(
        (veil.origin, veil.size),
        (pane.origin, pane.size),
        "the veil covers the whole tab, the composer included"
    );

    // A drag that leaves the window takes the veil with it.
    cx.simulate_event(FileDropEvent::Exited);
    assert!(
        cx.debug_bounds(super::view::DROP_VEIL).is_none(),
        "no veil once the drag leaves"
    );

    // A drag that moves off the tab, the window still there, clears it too.
    cx.simulate_event(FileDropEvent::Entered {
        position: pane.center(),
        paths: dragged(),
    });
    assert!(cx.debug_bounds(super::view::DROP_VEIL).is_some());
    cx.simulate_event(FileDropEvent::Pending {
        position: point(pane.origin.x + pane.size.width + px(200.), pane.center().y),
    });
    assert!(
        cx.debug_bounds(super::view::DROP_VEIL).is_none(),
        "a drag off the tab takes the veil with it"
    );
    cx.simulate_event(FileDropEvent::Ended);

    // Dropped on the tab: attached, and the veil goes with the drop.
    cx.simulate_event(FileDropEvent::Entered {
        position: pane.center(),
        paths: dragged(),
    });
    assert!(cx.debug_bounds(super::view::DROP_VEIL).is_some());
    cx.simulate_event(FileDropEvent::Submit {
        position: pane.center(),
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(super::view::DROP_VEIL).is_none(),
        "the veil goes once the files are dropped"
    );
    assert_eq!(files(&view, cx), vec![report], "the drop attached the file");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A tab beside sources of file-tree drags, as the tree's rows are: these
/// tests start the drag there and land it on the tab.
struct DragHost {
    tab: Entity<SessionView>,
    file: FileDrag,
    folder: FileDrag,
}

impl Render for DragHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(
                v_flex()
                    .w(px(120.))
                    .h_full()
                    .child(drag_row("file-tree-like-file", self.file.clone()))
                    .child(drag_row("file-tree-like-folder", self.folder.clone())),
            )
            .child(div().flex_1().h_full().child(self.tab.clone()))
    }
}

/// A row that drags `drag`, as the tree's own rows do.
fn drag_row(selector: &'static str, drag: FileDrag) -> impl IntoElement {
    div()
        .id(selector)
        .debug_selector(move || selector.into())
        .h(px(24.))
        .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
}

/// A window with the tab beside a file row and a folder row it can be dragged
/// from.
fn drag_host(
    cx: &mut TestAppContext,
    file: std::path::PathBuf,
    folder: std::path::PathBuf,
) -> (Entity<DragHost>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let cwd = std::env::temp_dir();
    let mut built = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let tab = cx.new(|cx| {
            let mut tab = SessionView::new(cwd, None, window, cx, false);
            tab.backend.started = true;
            tab
        });
        let host = cx.new(|_| DragHost {
            tab,
            file: FileDrag::new(file, "report.pdf", false),
            folder: FileDrag::new(folder, "notes", true),
        });
        built = Some(host.clone());
        // The window's root, so a warning (a refused folder) can be shown.
        gpui_kit::base::Root::new(host, window, cx)
    });
    (built.expect("the drag host was built"), cx)
}

/// Drag from the row drawn as `from` toward the middle of the tab, in steps,
/// as a hand would; the caller lets go (a `mouse_up` on the target).
fn drag_onto_the_tab(cx: &mut VisualTestContext, from: &'static str) {
    let start = bounds(cx, from).center();
    let to = bounds(cx, super::view::DROP_TARGET).center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    for step in 1..=6 {
        let t = step as f32 / 6.;
        let at = point(
            start.x + (to.x - start.x) * t,
            start.y + (to.y - start.y) * t,
        );
        cx.simulate_mouse_move(at, Some(MouseButton::Left), Modifiers::none());
        cx.run_until_parked();
    }
}

/// S12: a file dragged out of the tree onto the tab raises the same veil a
/// drag from Finder does and attaches like it; a folder row is refused there
/// too, so it adds no card.
#[gpui::test]
fn dragging_a_file_out_of_the_tree_attaches_it(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("gs-tree-drag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.pdf");
    std::fs::write(&report, b"%PDF-1.7\n").unwrap();
    let notes = dir.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    let (host, cx) = drag_host(cx, report.clone(), notes);

    drag_onto_the_tab(cx, "file-tree-like-file");
    assert!(
        cx.debug_bounds(super::view::DROP_VEIL).is_some(),
        "the tree's drag raises the tab's veil"
    );
    let target = bounds(cx, super::view::DROP_TARGET).center();
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(super::view::DROP_VEIL).is_none(),
        "the veil goes with the drop"
    );
    let tab = host.read_with(cx, |host, _| host.tab.clone());
    assert_eq!(
        files(&tab, cx),
        vec![report],
        "dropping the dragged file attached it"
    );

    drag_onto_the_tab(cx, "file-tree-like-folder");
    assert!(
        cx.debug_bounds(super::view::DROP_VEIL).is_some(),
        "the folder drag raises it too: the tab refuses on the rules, not the target"
    );
    let target = bounds(cx, super::view::DROP_TARGET).center();
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        files(&tab, cx),
        vec![dir.join("report.pdf")],
        "a folder is refused, so it adds no card"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

// ── The `/` menu (S2, S3) ──────────────────────────────────────────────────

/// A tab whose `/` menu offers `names`, and the screens it asked its window
/// to open, in order.
struct SlashTab {
    view: Entity<SessionView>,
    screens: Rc<RefCell<Vec<Screen>>>,
    _opened: Subscription,
}

impl SlashTab {
    /// The screens the window was asked to open.
    fn opened(&self) -> Vec<Screen> {
        self.screens.borrow().clone()
    }
}

fn slash_tab<'a>(
    names: &[&str],
    cx: &'a mut TestAppContext,
) -> (SlashTab, &'a mut VisualTestContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, _| {
        view.commands = names
            .iter()
            .map(|name| ((*name).to_string(), format!("what /{name} does")))
            .collect();
    });
    let screens: Rc<RefCell<Vec<Screen>>> = Rc::default();
    let opened = screens.clone();
    let opened = cx.cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &super::SessionEvent, _| {
            if let super::SessionEvent::OpenScreen(screen) = event {
                opened.borrow_mut().push(*screen);
            }
        })
    });
    (
        SlashTab {
            view,
            screens,
            _opened: opened,
        },
        cx,
    )
}

/// Put `text` in the composer as typing it leaves it: focused, with the `/`
/// menu up for it.
fn type_in_composer(view: &Entity<SessionView>, cx: &mut VisualTestContext, text: &str) {
    let end = text.chars().map(char::len_utf16).sum::<usize>() as u32;
    cx.update(|window, cx| {
        let composer = view.read(cx).composer.clone();
        composer.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_value(text, window, cx);
            state.set_cursor_position(Position::new(0, end), window, cx);
        });
    });
    cx.run_until_parked();
}

/// The prompts the conversation holds: what a send would have added.
fn sent_prompts(view: &Entity<SessionView>, cx: &mut VisualTestContext) -> usize {
    view.read_with(cx, |view, _| {
        view.transcript
            .items
            .iter()
            .filter(|item| matches!(item, gentle_core::transcript::Item::User { .. }))
            .count()
    })
}

/// The composer's inline tokens as `(id, text, range)`.
fn composer_tokens(
    view: &Entity<SessionView>,
    cx: &mut VisualTestContext,
) -> Vec<(String, String, std::ops::Range<usize>)> {
    view.read_with(cx, |view, cx| {
        view.composer
            .read(cx)
            .tokens()
            .iter()
            .map(|span| {
                (
                    span.token().id().to_string(),
                    span.token().text().to_string(),
                    span.range(),
                )
            })
            .collect()
    })
}

/// S2: one Enter on the highlighted command opens its screen, as sending the
/// command does: it neither fills the text in to wait for a second press nor
/// sends anything twice. Tab still only fills it in.
#[gpui::test]
fn enter_opens_a_highlighted_native_screen_once(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs", "gentle:profiles"], cx);
    type_in_composer(&tab.view, cx, "/gentle:pro");

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    assert_eq!(
        tab.opened(),
        vec![Screen::Profiles],
        "one press opens the screen, once"
    );
    assert_eq!(
        composer_text(&tab.view, cx),
        "",
        "the command left the composer, as a sent one does"
    );
    assert_eq!(sent_prompts(&tab.view, cx), 0, "nothing was sent to Pi");
    assert!(
        tab.view.read_with(cx, |view, cx| !view.slash_menu_open(cx)),
        "the menu is gone with the text"
    );

    // The screen is open and the composer is empty: another Enter sends
    // nothing more, so one press can never dispatch twice.
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(tab.opened(), vec![Screen::Profiles], "and only once");
}

/// S2: Tab completes a command with a native screen without opening it, and
/// Enter fills in a command that has none (a built-in runs on the next
/// press, as before).
#[gpui::test]
fn tab_completes_without_opening_and_other_commands_still_only_fill_in(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["gentle:profiles"], cx);
    type_in_composer(&tab.view, cx, "/gentle:pro");
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(composer_text(&tab.view, cx), "/gentle:profiles ");
    assert!(tab.opened().is_empty(), "Tab completes, it does not open");

    type_in_composer(&tab.view, cx, "/comp");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        composer_text(&tab.view, cx),
        "/compact ",
        "a built-in is filled in, not run"
    );
    assert!(tab.opened().is_empty());
    assert_eq!(sent_prompts(&tab.view, cx), 0);
}

/// S2: a command typed in full still opens its screen on one Enter, as it did.
#[gpui::test]
fn a_command_typed_in_full_still_opens_on_enter(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["gentle:profiles"], cx);
    type_in_composer(&tab.view, cx, "/gentle:profiles");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(tab.opened(), vec![Screen::Profiles]);
    assert_eq!(composer_text(&tab.view, cx), "");
}

/// S3: picking a skill leaves its own `/skill:name` in the prompt as one
/// inline reference marked for the composer, the caret after it for the
/// instructions; nothing opens and nothing is sent.
#[gpui::test]
fn picking_a_skill_inserts_its_reference_and_waits_for_instructions(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs", "gentle:profiles"], cx);
    type_in_composer(&tab.view, cx, "/skill:arch");

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    assert_eq!(composer_text(&tab.view, cx), "/skill:architecture-docs ");
    assert_eq!(
        composer_tokens(&tab.view, cx),
        vec![(
            "skill-reference".to_string(),
            "/skill:architecture-docs".to_string(),
            0.."/skill:architecture-docs".len(),
        )],
        "the command itself, kept as one marked reference"
    );
    let cursor = tab
        .view
        .read_with(cx, |view, cx| view.composer.read(cx).cursor());
    assert_eq!(
        cursor,
        "/skill:architecture-docs ".len(),
        "the caret waits where the instructions go"
    );
    assert!(tab.opened().is_empty(), "a skill opens nothing");
    assert_eq!(sent_prompts(&tab.view, cx), 0, "and nothing is sent");
    assert!(tab.view.read_with(cx, |view, cx| !view.slash_menu_open(cx)));

    // Typed at the caret: the instructions follow the reference, which stays
    // one reference in the prompt.
    tab.view.update_in(cx, |view, window, cx| {
        view.composer.update(cx, |state, cx| {
            state.replace("summarize the ADRs", window, cx)
        });
    });
    cx.run_until_parked();
    assert_eq!(
        composer_text(&tab.view, cx),
        "/skill:architecture-docs summarize the ADRs"
    );
    assert_eq!(composer_tokens(&tab.view, cx).len(), 1);
}

/// S2 and S3: a click on a row does what Enter on it does, so the pointer and
/// the keyboard leave the composer in the same state.
#[gpui::test]
fn clicking_a_row_does_what_enter_does(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs", "gentle:profiles"], cx);

    type_in_composer(&tab.view, cx, "/skill:arch");
    let row = bounds(cx, "slash-command-0").center();
    cx.simulate_mouse_down(row, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(row, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(composer_text(&tab.view, cx), "/skill:architecture-docs ");
    assert_eq!(composer_tokens(&tab.view, cx).len(), 1);
    assert!(tab.opened().is_empty());

    type_in_composer(&tab.view, cx, "/gentle:pro");
    let row = bounds(cx, "slash-command-0").center();
    cx.simulate_mouse_down(row, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(row, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(tab.opened(), vec![Screen::Profiles]);
    assert_eq!(composer_text(&tab.view, cx), "");
}

/// S3: a pick replaces only the `/query` at the caret — `/skill:arch`, or the
/// `/arch` the menu matched by name — leaving the prose and a paste's chip
/// around it; with no query at the caret nothing is picked.
#[gpui::test]
fn picking_a_skill_replaces_only_the_query_at_the_caret(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs"], cx);
    let item = gpui_kit::ClipboardItem::new_string("fn main() {}\n".repeat(8));
    let (prefix, rest, keep) = ("[Pasted text #1] ", "keep /skill:arch and this", "keep ");
    let name = "/skill:architecture-docs";
    let caret = format!("{prefix}{keep}/skill:arch").len() as u32;
    tab.view.update_in(cx, |view, window, cx| {
        view.paste_clipboard(&item, window, cx);
        view.composer.update(cx, |state, cx| {
            state.replace(rest, window, cx);
            state.set_cursor_position(Position::new(0, caret), window, cx)
        });
        view.insert_skill_reference("skill:architecture-docs", window, cx);
    });
    let picked = format!("{prefix}{keep}{name} and this");
    assert_eq!(composer_text(&tab.view, cx), picked);
    let (tokens, start) = (composer_tokens(&tab.view, cx), prefix.len() + keep.len());
    assert_eq!(tokens.len(), 2, "the paste's chip and the skill's");
    assert_eq!(tokens[0].0, "pasted-text");
    assert_eq!(tokens[1].0, "skill-reference");
    assert_eq!(tokens[1].1, name);
    assert_eq!(tokens[1].2, start..start + name.len());
    // No query at the caret: nothing is picked.
    type_in_composer(&tab.view, cx, "draft");
    tab.view.update_in(cx, |view, window, cx| {
        view.insert_skill_reference("skill:docs", window, cx)
    });
    assert_eq!(composer_text(&tab.view, cx), "draft");
    type_in_composer(&tab.view, cx, "/arch");
    cx.simulate_keystrokes("enter");
    let completed = format!("{name} ");
    assert_eq!(composer_text(&tab.view, cx), completed);
    assert_eq!(composer_tokens(&tab.view, cx).len(), 1);
}

/// S3: the reference keeps its marking when the composer is rebuilt from its
/// text — removing a paste card takes that chip out and leaves the skill's.
#[gpui::test]
fn removing_a_paste_card_keeps_the_skill_reference_marked(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs"], cx);
    type_in_composer(&tab.view, cx, "/skill:arch");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    let item = gpui_kit::ClipboardItem::new_string("fn main() {}\n".repeat(8));
    tab.view.update_in(cx, |view, window, cx| {
        view.paste_clipboard(&item, window, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        composer_text(&tab.view, cx),
        "/skill:architecture-docs [Pasted text #1] "
    );

    tab.view
        .update_in(cx, |view, window, cx| view.remove_pasted(0, window, cx));
    cx.run_until_parked();
    assert_eq!(
        composer_text(&tab.view, cx),
        "/skill:architecture-docs ",
        "the paste's card and token went"
    );
    assert_eq!(
        composer_tokens(&tab.view, cx),
        vec![(
            "skill-reference".to_string(),
            "/skill:architecture-docs".to_string(),
            0.."/skill:architecture-docs".len(),
        )],
        "the skill keeps its marking through the rebuild"
    );
}

/// S3: a prompt recalled with ↑ keeps the marking of the skill it invokes,
/// and leaves a lookalike the backend never listed as plain text.
#[gpui::test]
fn a_recalled_prompt_keeps_its_skill_reference_marked(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs"], cx);
    tab.view.update(cx, |view, _| {
        view.prompt_history
            .record("/skill:architecture-docs summarize the ADRs");
    });
    type_in_composer(&tab.view, cx, "draft");

    cx.simulate_keystrokes("up");
    cx.run_until_parked();
    assert_eq!(
        composer_text(&tab.view, cx),
        "/skill:architecture-docs summarize the ADRs"
    );
    assert_eq!(
        composer_tokens(&tab.view, cx),
        vec![(
            "skill-reference".to_string(),
            "/skill:architecture-docs".to_string(),
            0.."/skill:architecture-docs".len(),
        )],
        "the recalled command is still the known skill's reference"
    );
}

/// S3: a command that only looks like a skill stays plain — one the backend
/// never listed, a listed one with more letters after it, and one glued to
/// the word before it.
#[gpui::test]
fn lookalike_commands_stay_plain_through_a_rebuild(cx: &mut TestAppContext) {
    let (tab, cx) = slash_tab(&["skill:architecture-docs"], cx);
    for prompt in [
        "/skill:unknown-thing summarize",
        "/skill:architecture-docsx summarize",
        "see/skill:architecture-docs",
        "/skills:architecture-docs",
    ] {
        tab.view.update(cx, |view, _| {
            view.prompt_history.record(prompt);
        });
        type_in_composer(&tab.view, cx, "draft");
        cx.simulate_keystrokes("up");
        cx.run_until_parked();
        assert_eq!(composer_text(&tab.view, cx), prompt);
        assert!(
            composer_tokens(&tab.view, cx).is_empty(),
            "{prompt} is no known skill's reference"
        );
        // Back to the draft, ready for the next prompt.
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        assert_eq!(composer_text(&tab.view, cx), "draft");
    }
}

/// A long paste keeps its own `#N` chip (the token the skill reference shares
/// the renderer with) and still goes out expanded.
#[gpui::test]
fn a_long_paste_keeps_its_chip(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let pasted = "fn main() {}\n".repeat(8);
    let item = gpui_kit::ClipboardItem::new_string(pasted);
    let kept = view.update_in(cx, |view, window, cx| {
        view.paste_clipboard(&item, window, cx)
    });
    cx.run_until_parked();
    assert!(kept, "eight lines are a long paste");
    assert_eq!(composer_text(&view, cx), "[Pasted text #1] ");
    assert_eq!(
        composer_tokens(&view, cx),
        vec![(
            "pasted-text".to_string(),
            "[Pasted text #1]".to_string(),
            0.."[Pasted text #1]".len(),
        )]
    );
}

/// A stand-in for `gentle-shell --mode rpc`: it appends every command it
/// reads to the file named by its first argument and answers each with
/// success; `get_messages` returns a conversation of one user message.
#[cfg(unix)]
const FAKE_BACKEND: &str = r#"while IFS= read -r line; do
  printf '%s\n' "$line" >> "$1"
  id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_messages"'*) data='{"messages":[{"role":"user","content":"hello"}]}' ;;
    *) data='{}' ;;
  esac
  printf '{"type":"response","id":"%s","success":true,"data":%s}\n' "$id" "$data"
done"#;

/// A tab whose backend is [`FAKE_BACKEND`]; its commands go to `log`.
#[cfg(unix)]
struct Backend {
    dir: std::path::PathBuf,
    log: std::path::PathBuf,
    incoming: async_channel::Receiver<pi_rpc::Incoming>,
}

#[cfg(unix)]
impl Backend {
    fn start(view: &Entity<SessionView>, cx: &mut VisualTestContext) -> Self {
        // The answers come from the process's reader thread.
        cx.executor().allow_parking();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("gs-fake-backend-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("commands.jsonl");
        let spec = pi_rpc::LaunchSpec {
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                FAKE_BACKEND.into(),
                "sh".into(),
                log.to_string_lossy().into_owned(),
            ],
            cwd: dir.clone(),
            env: Vec::new(),
            launcher: "/bin/sh".into(),
            bundled: None,
        };
        let (process, incoming) = pi_rpc::PiProcess::spawn(&spec).expect("spawn the fake backend");
        view.update(cx, |view, _| {
            view.backend.status = super::state::BackendStatus::Ready(process);
        });
        Self { dir, log, incoming }
    }

    /// The `type` of every command the backend has read so far.
    fn read(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .map(|command| command["type"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Run the tab until `ready` holds for what the backend has read, then
    /// every command sent before a marker, with the answers to all of them
    /// applied: the pipe keeps their order, so once the marker is answered
    /// nothing sent earlier is still on its way.
    fn settle(
        &self,
        view: &Entity<SessionView>,
        cx: &mut VisualTestContext,
        ready: impl Fn(&[String]) -> bool,
    ) -> Vec<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !ready(&self.read()) {
            assert!(
                std::time::Instant::now() < deadline,
                "the backend never got there; it read {:?}",
                self.read()
            );
            cx.run_until_parked();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let process = view
            .read_with(cx, |view, _| view.process())
            .expect("a backend");
        smol::block_on(process.request(json!({"type": "marker"}))).expect("the marker's answer");
        cx.run_until_parked();
        let mut read = self.read();
        let marker = read
            .iter()
            .position(|command| command == "marker")
            .expect("the marker");
        read.truncate(marker);
        read
    }

    /// Stop the process, wait until it has exited and remove its folder.
    fn stop(self, view: &Entity<SessionView>, cx: &mut VisualTestContext) {
        let status = view.update(cx, |view, _| {
            std::mem::replace(
                &mut view.backend.status,
                super::state::BackendStatus::Exited(None),
            )
        });
        if let super::state::BackendStatus::Ready(process) = status {
            process.shutdown(std::time::Duration::ZERO);
        }
        while let Ok(incoming) = self.incoming.recv_blocking() {
            if matches!(incoming, pi_rpc::Incoming::Exited(_)) {
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(unix)]
fn user_texts(view: &Entity<SessionView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |view, _| {
        view.transcript
            .items
            .iter()
            .filter_map(|item| match item {
                gentle_core::transcript::Item::User { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    })
}

/// Changing the model or the thinking level (the Profiles screen) re-reads
/// the session's state, but not its conversation, which they cannot change.
#[cfg(unix)]
#[gpui::test]
fn a_model_or_thinking_change_does_not_reload_the_conversation(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let backend = Backend::start(&view, cx);
    view.update(cx, |view, cx| {
        view.send_command(
            json!({"type": "set_model", "provider": "anthropic", "modelId": "claude"}),
            cx,
        );
        view.send_command(json!({"type": "set_thinking_level", "level": "high"}), cx);
    });
    let read = backend.settle(&view, cx, |read| {
        read.iter()
            .filter(|command| *command == "get_state")
            .count()
            == 2
    });
    assert_eq!(read[..2], ["set_model", "set_thinking_level"]);
    assert!(
        !read.iter().any(|command| command == "get_messages"),
        "it read {read:?}"
    );
    assert!(
        user_texts(&view, cx).is_empty(),
        "the conversation was rebuilt"
    );
    backend.stop(&view, cx);
}

/// Opening a saved session still loads its conversation.
#[cfg(unix)]
#[gpui::test]
fn opening_a_saved_session_loads_its_conversation(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let backend = Backend::start(&view, cx);
    let saved = backend.dir.join("saved.jsonl");
    view.update(cx, |view, cx| view.open_session(saved.clone(), cx));
    let read = backend.settle(&view, cx, |read| {
        read.iter().any(|command| command == "get_state")
    });
    assert_eq!(read[0], "switch_session");
    assert!(
        read.iter().any(|command| command == "get_messages"),
        "it read {read:?}"
    );
    assert_eq!(user_texts(&view, cx), ["hello"]);
    backend.stop(&view, cx);
}

/// The conversation a notice is tied to: ⌘N and opening another session
/// replace it as soon as they are asked (the new session's startup notices
/// arrive before the answer); resuming the one shown (a restart) keeps it.
#[cfg(unix)]
#[gpui::test]
fn only_another_session_replaces_the_conversation(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let backend = Backend::start(&view, cx);
    let (shown, other) = (backend.dir.join("a.jsonl"), backend.dir.join("b.jsonl"));
    let get_states =
        |n: usize| move |read: &[String]| read.iter().filter(|c| *c == "get_state").count() >= n;
    let conversation =
        |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.conversation());

    view.update(cx, |view, cx| {
        view.session_file = Some(shown.clone());
        view.open_session(shown.clone(), cx);
    });
    backend.settle(&view, cx, get_states(1));
    assert_eq!(conversation(cx), 0, "resuming the one shown");

    view.update_in(cx, |view, window, cx| view.new_session(window, cx));
    assert_eq!(conversation(cx), 1, "counted when asked");
    backend.settle(&view, cx, get_states(2));
    assert_eq!(conversation(cx), 1);

    view.update(cx, |view, cx| {
        view.session_file = Some(shown.clone());
        view.open_session(other.clone(), cx);
    });
    backend.settle(&view, cx, get_states(3));
    assert_eq!(conversation(cx), 2);
    backend.stop(&view, cx);
}

/// A tab as the workspace pane draws it: the agent's view, cached, with its
/// live layer over it.
#[cfg(unix)]
struct Pane(Entity<SessionView>);

#[cfg(unix)]
impl gpui_kit::Render for Pane {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, StyleRefinement, Styled as _};
        gpui_kit::div()
            .relative()
            .size_full()
            .child(
                self.0
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
            .child(self.0.read(cx).live_layer())
    }
}

/// A pane whose tab has a ready backend and its composer focused, with
/// typing kisses on (the default) unless `reduce_motion`.
#[cfg(unix)]
fn typing_pane(
    cx: &mut TestAppContext,
    reduce_motion: bool,
) -> (Entity<SessionView>, &mut VisualTestContext, Backend) {
    use gpui_kit::AppContext as _;
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(reduce_motion);
    });
    let cwd = std::env::temp_dir();
    let (pane, cx) = cx.add_window_view(|window, cx| {
        Pane(cx.new(|cx| {
            let mut view = SessionView::new(cwd, None, window, cx, false);
            view.backend.started = true;
            view
        }))
    });
    let view = pane.read_with(cx, |pane, _| pane.0.clone());
    cx.update(|window, _| window.activate_window());
    let backend = Backend::start(&view, cx);
    cx.update(|window, cx| {
        let composer = view.read(cx).composer.clone();
        composer.update(cx, |state, cx| state.focus(window, cx));
    });
    cx.run_until_parked();
    // The focus pulse plays out first.
    frames_for(std::time::Duration::from_millis(600), cx);
    assert_eq!(cx.update(|window, cx| window.simulate_next_frame(cx)), 0);
    (view, cx, backend)
}

/// Deliver display frames, one every 16 ms, for `span`; how many of them
/// something asked for.
#[cfg(unix)]
fn frames_for(span: std::time::Duration, cx: &mut VisualTestContext) -> usize {
    let start = std::time::Instant::now();
    let mut asked = 0;
    while start.elapsed() < span {
        std::thread::sleep(std::time::Duration::from_millis(16));
        asked += cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    asked
}

/// Typing blows a kiss from the caret; it flies while it lives (frames are
/// asked for) and lands after its 900 ms (none are).
#[cfg(unix)]
#[gpui::test]
fn a_typed_kiss_rises_from_the_caret_and_lands(cx: &mut TestAppContext) {
    let (view, cx, backend) = typing_pane(cx, false);
    let caret = view.read_with(cx, |view, cx| {
        let caret = view.composer.read(cx).prepaint_caret_geometry();
        caret
            .expect("the composer was drawn")
            .cursor_bounds()
            .origin
    });
    cx.simulate_keystrokes("a");
    let blown: Vec<_> = view.read_with(cx, |view, _| {
        view.kisses.iter().map(|(_, at, _)| *at).collect()
    });
    assert_eq!(blown, [caret]);
    assert!(frames_for(std::time::Duration::from_millis(200), cx) > 0);
    frames_for(std::time::Duration::from_millis(900), cx);
    assert_eq!(cx.update(|window, cx| window.simulate_next_frame(cx)), 0);
    backend.stop(&view, cx);
}

/// With reduced motion the composer blows no kiss and asks for no frame.
#[cfg(unix)]
#[gpui::test]
fn reduced_motion_blows_no_kiss(cx: &mut TestAppContext) {
    let (view, cx, backend) = typing_pane(cx, true);
    cx.simulate_keystrokes("a");
    assert!(view.read_with(cx, |view, _| view.kisses.is_empty()));
    assert_eq!(cx.update(|window, cx| window.simulate_next_frame(cx)), 0);
    backend.stop(&view, cx);
}

/// A kiss's flight redraws only the tab's live layer: the conversation is
/// drawn for the keystroke and reused from its cache while the kiss flies.
#[cfg(unix)]
#[gpui::test]
fn a_typed_kiss_redraws_only_the_live_layer(cx: &mut TestAppContext) {
    let (view, cx, backend) = typing_pane(cx, false);
    let renders = || super::view::RENDERS.with(std::cell::Cell::get);
    cx.simulate_keystrokes("a");
    // The keystroke draws the tab, and so does the next frame: an edit
    // leaves it to draw once more, as the text area reports its moved caret
    // while it paints (gpui-base's input element), kiss or no kiss.
    assert!(cx.update(|window, cx| window.simulate_next_frame(cx)) > 0);
    cx.run_until_parked();
    let flying = renders();
    assert!(frames_for(std::time::Duration::from_secs(1), cx) > 0);
    assert_eq!(
        renders() - flying,
        0,
        "SessionView renders during the flight"
    );
    assert_eq!(cx.update(|window, cx| window.simulate_next_frame(cx)), 0);
    backend.stop(&view, cx);
}

/// `n` numbered lines.
fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// A loaded conversation's tool and shell cards show their title, subject
/// and line counts.
#[gpui::test]
fn loaded_tool_and_shell_cards_show_their_title_subject_and_line_count(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update(cx, |view, cx| {
        view.transcript.load_messages(&[
            json!({"role": "user", "content": "run them"}),
            json!({"role": "assistant", "stopReason": "toolUse", "content": [
                {"type": "text", "text": "Running the tests."},
                {"type": "toolCall", "id": "t1", "name": "bash",
                 "arguments": {"command": "cargo test", "description": "Run the tests"}},
            ]}),
            json!({"role": "toolResult", "toolCallId": "t1", "isError": false,
                   "content": [{"type": "text", "text": lines(20)}], "details": {}}),
            json!({"role": "assistant", "stopReason": "stop",
                   "content": [{"type": "text", "text": "Done."}]}),
            json!({"role": "bashExecution", "command": "git status",
                   "output": lines(12), "exitCode": 0}),
        ]);
        cx.notify();
    });
    // The finished step shows its quiet line; the shell its card.
    assert!(drawn(cx, "step-row:Run the tests:cargo test"));
    assert!(drawn(cx, "step-row:Shell:"));
    assert!(drawn(cx, "fold:4 earlier lines"));
    assert!(!drawn(cx, "fold:12 earlier lines"));

    click(cx, "step-row:Run the tests:cargo test");
    assert!(drawn(cx, "fold:12 earlier lines"));
}

/// A running tool's card follows its output: the line count it shows is the
/// latest output's.
#[gpui::test]
fn a_running_tools_card_shows_its_growing_output(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    let update = |text: String| {
        json!({"type": "tool_execution_update", "toolCallId": "r1",
               "partialResult": {"content": [{"type": "text", "text": text}]}})
    };
    view.update(cx, |view, cx| {
        view.transcript
            .apply(&json!({"type": "tool_execution_start", "toolCallId": "r1",
            "toolName": "bash", "args": {"command": "npm test", "description": "Run npm"}}));
        view.transcript.apply(&update(lines(10)));
        cx.notify();
    });
    click(cx, "step-row:Run npm:npm test");
    assert!(drawn(cx, "fold:2 earlier lines"));

    view.update(cx, |view, cx| {
        view.transcript.apply(&update(lines(30)));
        cx.notify();
    });
    assert!(drawn(cx, "fold:22 earlier lines"));
    assert!(!drawn(cx, "fold:2 earlier lines"));
}

// Idle structured updates change the chrome only when the running counts do,
// so the workspace's background light follows them without a turn.
#[gpui::test]
fn idle_background_updates_emit_chrome_changed(cx: &mut TestAppContext) {
    use std::{cell::RefCell, rc::Rc};

    let (view, cx) = open(&[], cx);
    cx.run_until_parked();
    view.update(cx, |view, cx| view.sync_chrome(cx));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observed = seen.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&view, move |view, event: &super::SessionEvent, cx| {
            if matches!(event, super::SessionEvent::ChromeChanged) {
                let view = view.read(cx);
                assert!(!view.is_running());
                let background = view.background();
                observed
                    .borrow_mut()
                    .push((background.subagents, background.jobs));
            }
        })
    });

    for (seq, event, data) in [
        (
            2,
            "agents_update",
            json!({"agents": [{
                "id": "agent-1", "agent": "worker", "status": "queued",
                "lastStep": "writing", "usage": {"total": 100, "cost": 0.01}
            }]}),
        ),
        (
            3,
            "agents_update",
            json!({"agents": [{
                "id": "agent-1", "agent": "worker", "status": "running",
                "lastStep": "writing", "usage": {"total": 100, "cost": 0.01}
            }]}),
        ),
        (
            4,
            "agents_update",
            json!({"agents": [{
                "id": "agent-1", "agent": "worker", "status": "waiting",
                "lastStep": "writing", "usage": {"total": 100, "cost": 0.01}
            }]}),
        ),
        (5, "jobs_update", json!({"jobs": []})),
    ] {
        view.update_in(cx, |view, window, cx| {
            view.handle_batch(
                vec![pi_rpc::Incoming::Ui(widget(json!({
                    "schema": SCHEMA, "kind": "event", "seq": seq,
                    "event": event, "data": data,
                })))],
                window,
                cx,
            );
        });
        cx.run_until_parked();
    }
    // The harness seeds one running job; a queued or waiting subagent
    // doesn't count, a running one does, and the emptied job list clears it.
    let mut seen = seen.borrow().clone();
    seen.dedup();
    assert_eq!(seen, [(0, 1), (1, 1), (0, 1), (0, 0)]);
}

// A job last seen running must not keep the light on once the backend exits.
#[gpui::test]
fn backend_exit_clears_the_background_light(cx: &mut TestAppContext) {
    let (view, cx) = tab(cx);
    view.update_in(cx, |view, window, cx| {
        view.handle_batch(
            vec![pi_rpc::Incoming::Event(json!({
                "type": "tool_execution_end", "toolName": "bash_background",
                "result": {"details": {"gentleJobs": {"jobId": 7, "status": "running"}}},
            }))],
            window,
            cx,
        );
        assert_eq!(view.chrome.as_ref().unwrap().background.jobs, 1);
        view.handle_batch(vec![pi_rpc::Incoming::Exited(None)], window, cx);
        assert!(view.chrome.as_ref().unwrap().background.is_idle());
    });
}
