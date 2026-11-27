//! Running a `gentle-browser` operation in a page: the page script
//! (`automation.js`) is loaded on demand, an operation is started in it and
//! its answer polled, since a page's promises can't be awaited from outside.
//! What doesn't happen inside the page (navigating, the screenshot) goes
//! through the native view. The window side (finding, opening and closing
//! pages) is `workspace::stages`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gentle_core::browser::{Action, Op, Target, error, eval_script, ok, run_script, take_script};
use gpui_kit::*;
use serde_json::{Value, json};

use super::WebSurface;

const SCRIPT: &str = include_str!("automation.js");
/// How often a running operation is asked for its answer.
const POLL: Duration = Duration::from_millis(100);
/// How long one script may take to answer at all.
const EVAL: Duration = Duration::from_secs(5);

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Run `op` (not `open`, `tabs`, `close` or `release`: the window does those)
/// in `surface`'s page; the JSON answer.
pub async fn run(op: Op, surface: Entity<WebSurface>, cx: &mut AsyncWindowContext) -> Value {
    if let Err(answer) = running(&surface, cx).await {
        return answer;
    }
    let deadline = Instant::now() + Duration::from_secs_f64(op.timeout());
    let result = match op {
        Op::Goto { url, .. } => goto(&surface, url, deadline, cx).await,
        Op::Back { .. } => history(&surface, -1, deadline, cx).await,
        Op::Forward { .. } => history(&surface, 1, deadline, cx).await,
        Op::Reload { .. } => match mark(&surface, cx).await {
            Ok(mark) => {
                surface.update(cx, |s, cx| s.reload(cx));
                left(&surface, &mark, cx).await;
                settle(&surface, deadline, cx).await.map(ok)
            }
            Err(answer) => Err(answer),
        },
        Op::State { .. } => call(&surface, "state", json!([]), deadline, cx).await,
        Op::Eval { js, .. } => {
            let id = next_id();
            started(&surface, &id, &eval_script(&id, &js), deadline, cx).await
        }
        Op::Snapshot {
            root, text, nodes, ..
        } => {
            let opts = json!({
                "root": root.map(|r| Target::parse(&r).to_json()),
                "text": text,
                "nodes": nodes,
            });
            call(&surface, "snapshot", json!([opts]), deadline, cx).await
        }
        Op::Act {
            action,
            target,
            text,
            dy,
            ..
        } => {
            if needs_target(action) && target.is_none() {
                return error("bad_op", format!("{action:?} needs a target"));
            }
            let target = target.map(|t| Target::parse(&t).to_json());
            let action = serde_json::to_value(action).unwrap_or(Value::Null);
            act(&surface, json!([action, target, text, dy]), deadline, cx).await
        }
        Op::Wait {
            target,
            gone,
            timeout,
            ..
        } => {
            let ms = (timeout.unwrap_or(10.).clamp(0.1, 120.) * 1000.) as u64;
            let args = json!([Target::parse(&target).to_json(), gone, ms]);
            call(&surface, "wait", args, deadline, cx).await
        }
        Op::Pick { .. } => call(&surface, "pick", json!([]), deadline, cx).await,
        Op::Screenshot { .. } => screenshot(&surface, cx).await,
        Op::Open { .. } | Op::Tabs | Op::Close { .. } | Op::Release { .. } => {
            Err(error("bad_op", "the window answers this one"))
        }
    };
    result.unwrap_or_else(|answer| answer)
}

fn next_id() -> String {
    format!("g{}", NEXT.fetch_add(1, Ordering::Relaxed))
}
/// A page an agent just opened: wait until it loaded; its state.
pub async fn opened(surface: Entity<WebSurface>, cx: &mut AsyncWindowContext) -> Value {
    if let Err(answer) = running(&surface, cx).await {
        return answer;
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    settle(&surface, deadline, cx)
        .await
        .map(ok)
        .unwrap_or_else(|answer| answer)
}

fn needs_target(action: Action) -> bool {
    !matches!(action, Action::Type | Action::Press | Action::Scroll)
}

/// Wait for the page's native view (a page just opened or out of sight).
async fn running(surface: &Entity<WebSurface>, cx: &mut AsyncWindowContext) -> Result<(), Value> {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let state = surface.update_in(cx, |s, window, cx| {
            s.ensure_running(window, cx);
            (s.is_running(), s.failure())
        });
        match state {
            Ok((true, _)) => return Ok(()),
            Ok((false, Some(failure))) => return Err(error("unsupported", failure)),
            Err(_) => return Err(error("closed", "the page was closed")),
            Ok((false, None)) if Instant::now() > until => {
                return Err(error("timeout", "the page didn't start"));
            }
            Ok((false, None)) => cx.background_executor().timer(POLL).await,
        }
    }
}

/// Run `script` in the page; its value, as JSON text.
async fn evaluate(
    surface: &Entity<WebSurface>,
    script: &str,
    cx: &mut AsyncWindowContext,
) -> Result<String, Value> {
    let (tx, rx) = async_channel::bounded(1);
    let started = surface.read_with(cx, |s, _| {
        s.evaluate_with(script, move |value| {
            let _ = tx.try_send(value);
        })
    });
    if !started {
        return Err(error("closed", "the page isn't running"));
    }
    let timer = cx.background_executor().timer(EVAL);
    smol::future::or(async { rx.recv().await.ok() }, async {
        timer.await;
        None
    })
    .await
    .ok_or_else(|| error("timeout", "the page didn't answer (still loading?)"))
}

/// The page script's string answer: the JSON value is a string.
fn as_text(raw: &str) -> String {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Load the page script (once per page) and run `script` after it.
async fn start(
    surface: &Entity<WebSurface>,
    script: &str,
    cx: &mut AsyncWindowContext,
) -> Result<(), Value> {
    let raw = evaluate(surface, &format!("{SCRIPT};\n{script}"), cx).await?;
    if as_text(&raw) == "started" {
        Ok(())
    } else {
        Err(error("failed", "the page didn't start the operation"))
    }
}

/// Poll operation `id` until it answers: `None` when the page navigated
/// away first (a click that followed a link).
async fn answer(
    surface: &Entity<WebSurface>,
    id: &str,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Option<Value>, Value> {
    loop {
        let raw = evaluate(surface, &take_script(id), cx).await;
        match raw.map(|raw| as_text(&raw)) {
            Ok(text) if text == "__gone__" => return Ok(None),
            Ok(text) if !text.is_empty() => {
                let reply: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                return if let Some(value) = reply.get("ok") {
                    Ok(Some(ok(value.clone())))
                } else {
                    Err(json!({ "error": reply.get("error").cloned().unwrap_or(Value::Null) }))
                };
            }
            // Still running; a page between loads doesn't answer at all.
            Ok(_) | Err(_) => {}
        }
        if Instant::now() > deadline {
            return Err(error("timeout", "the operation didn't finish in time"));
        }
        cx.background_executor().timer(POLL).await;
    }
}

/// Start `script` (operation `id`) and wait for its answer; the page
/// leaving meanwhile is an error.
async fn started(
    surface: &Entity<WebSurface>,
    id: &str,
    script: &str,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    start(surface, script, cx).await?;
    answer(surface, id, deadline, cx)
        .await?
        .ok_or_else(|| error("navigated", "the page navigated away before answering"))
}

async fn call(
    surface: &Entity<WebSurface>,
    name: &str,
    args: Value,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    let id = next_id();
    started(surface, &id, &run_script(&id, name, &args), deadline, cx).await
}

/// An action may navigate (a link, a form): that ends it well.
async fn act(
    surface: &Entity<WebSurface>,
    args: Value,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    let id = next_id();
    start(surface, &run_script(&id, "act", &args), cx).await?;
    match answer(surface, &id, deadline, cx).await? {
        Some(done) => Ok(done),
        None => settle(surface, deadline, cx).await.map(|state| {
            ok(json!({ "done": true, "navigated": true, "url": state["url"], "title": state["title"] }))
        }),
    }
}

/// Wait until the page has loaded: the page script answers and the
/// document is complete. Its state.
async fn settle(
    surface: &Entity<WebSurface>,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    // Leave a moment for a navigation that is starting to show.
    cx.background_executor().timer(POLL).await;
    loop {
        if let Ok(reply) = call(surface, "state", json!([]), deadline, cx).await
            && reply["ok"]["loading"] == Value::Bool(false)
        {
            return Ok(reply["ok"].clone());
        }
        if Instant::now() > deadline {
            return Err(error("timeout", "the page didn't finish loading"));
        }
        cx.background_executor().timer(POLL * 2).await;
    }
}

/// Mark the current document, so [`left`] can tell when a navigation
/// replaced it.
async fn mark(surface: &Entity<WebSurface>, cx: &mut AsyncWindowContext) -> Result<String, Value> {
    let id = next_id();
    let script = format!(
        "window.__gentleMark = {}; \"marked\"",
        Value::from(id.as_str())
    );
    evaluate(surface, &script, cx).await?;
    Ok(id)
}

/// Wait until the document marked `id` is gone (a navigation replaced it),
/// up to 3 s: one that stays was a same-document navigation (a hash, a
/// single-page app's route) or none at all.
async fn left(surface: &Entity<WebSurface>, id: &str, cx: &mut AsyncWindowContext) {
    let script = format!(
        "window.__gentleMark === {} ? \"same\" : \"new\"",
        Value::from(id)
    );
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        // A page between loads answers nothing, or only late.
        if let Ok(raw) = evaluate(surface, &script, cx).await
            && as_text(&raw) == "new"
        {
            return;
        }
        cx.background_executor().timer(POLL).await;
    }
}

async fn goto(
    surface: &Entity<WebSurface>,
    url: String,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(error("bad_url", "only http(s) pages open here"));
    }
    let mark = mark(surface, cx).await?;
    surface.update(cx, |s, cx| s.load(url, cx));
    left(surface, &mark, cx).await;
    settle(surface, deadline, cx).await.map(ok)
}

async fn history(
    surface: &Entity<WebSurface>,
    step: i32,
    deadline: Instant,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    let mark = mark(surface, cx).await?;
    let id = next_id();
    start(surface, &run_script(&id, "history", &json!([step])), cx).await?;
    left(surface, &mark, cx).await;
    settle(surface, deadline, cx).await.map(ok)
}

/// The page as a PNG file in the temporary folder.
async fn screenshot(
    surface: &Entity<WebSurface>,
    cx: &mut AsyncWindowContext,
) -> Result<Value, Value> {
    let (tx, rx) = async_channel::bounded(1);
    let started = surface.read_with(cx, |s, _| {
        s.screenshot(Box::new(move |png| {
            let _ = tx.try_send(png);
        }))
    });
    if !started {
        return Err(error("closed", "the page isn't running"));
    }
    let timer = cx.background_executor().timer(Duration::from_secs(15));
    let png = smol::future::or(async { rx.recv().await.ok() }, async {
        timer.await;
        None
    })
    .await
    .ok_or_else(|| error("timeout", "no picture of the page came"))?
    .map_err(|message| error("failed", message))?;
    let (width, height) = image::load_from_memory(&png)
        .map(|image| (image.width(), image.height()))
        .unwrap_or((0, 0));
    let path = std::env::temp_dir().join(format!(
        "gentle-browser-{}-{}.png",
        std::process::id(),
        next_id()
    ));
    std::fs::write(&path, &png).map_err(|e| error("failed", e.to_string()))?;
    Ok(ok(
        json!({ "path": path, "width": width, "height": height }),
    ))
}
