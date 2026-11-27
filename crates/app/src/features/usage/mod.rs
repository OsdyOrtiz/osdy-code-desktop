//! Usage: native `/gentle:usage` (provider subscription windows) and
//! `/gentle:stats` (token and cost totals: from Gentle Shell's own
//! `get_usage_stats` when the active agent tab's Shell offers it, otherwise
//! from the session files).
//!
//! Parsing mirrors gentle-pi's `lib/shell-usage.ts` and `lib/stats-collector.ts`.
//! Tokens and API keys only ever travel from `auth.json` into the HTTP request
//! header; they are never stored in view state, logged, or rendered.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::*;

use gentle_core::usage::stats::{StatsRange, StatsSummary, aggregate_stats};
use gentle_core::usage::subscription::{ProviderUsage, UsageLimit, UsageWindow};
use pi_rpc::desktop_rpc::{
    DesktopMethod, ProviderUsage as RpcProviderUsage, StatsScope, UsageLimit as RpcUsageLimit,
    UsageResetAt, UsageStats, UsageStatus, UsageWindow as RpcUsageWindow,
};

use crate::features::session::{
    READ_DECIDE_WAIT, ReadOutcome, ReadRequest, ReadSupport, ReadTicket, SessionView,
};

mod credentials;
mod fetch;
mod shell_stats;
mod stats;
mod view;

use credentials::{CardState, ProviderCard, discover, read_auth};
use fetch::run_fetch;
use stats::{MAX_SESSION_BYTES, StatsData, forget_sessions, load_sessions};

/// The local UTC offset in seconds, for views outside this feature that
/// render local times (the session's plan-limits chip).
pub(crate) use stats::local_offset_secs;
pub(crate) use view::subscription::updated_ago;

pub enum UsageEvent {
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Usage,
    Stats,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Metric {
    Tokens,
    Cost,
}

#[derive(Debug, PartialEq)]
enum UsageState {
    Loading,
    Failed(UsageText),
    Loaded {
        cards: Vec<ProviderCard>,
        others: Vec<String>,
        /// The empty-state copy when `cards` is empty: the RPC path never
        /// reuses the legacy sign-in prompt.
        empty: UsageEmpty,
    },
}

/// Keep app-owned messages untranslated until render, while passing external
/// diagnostics through verbatim.
#[derive(Clone, Debug, PartialEq)]
enum UsageText {
    Key(&'static str),
    Detail(&'static str, String),
    PathError(String, String),
    External(String),
}

impl From<&'static str> for UsageText {
    fn from(key: &'static str) -> Self {
        Self::Key(key)
    }
}

impl UsageText {
    fn render(&self) -> String {
        use crate::shared::i18n::t;
        match self {
            Self::Key(key) => t(key).to_string(),
            Self::Detail(key, detail) => t(key).replace("{detail}", detail),
            Self::PathError(path, error) => gentle_core::i18n::fill(
                t("Could not read {path}: {error}"),
                &[("path", path), ("error", error)],
            ),
            Self::External(text) => text.clone(),
        }
    }
}

/// What the Subscription tab shows when there are no provider cards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum UsageEmpty {
    /// Legacy path: no provider is signed in (`auth.json` has nothing).
    SignedIn,
    /// RPC path: the server reported no subscription usage/providers yet.
    Reported,
}

impl UsageEmpty {
    /// The empty-state line, by source path.
    fn message(&self) -> &'static str {
        match self {
            UsageEmpty::SignedIn => {
                "No subscription provider is signed in. Sign in to ChatGPT (Codex), Claude or NaN from Providers."
            }
            UsageEmpty::Reported => "This server has not reported any subscription usage yet.",
        }
    }
}

enum StatsState {
    Idle,
    Loading,
    Loaded {
        source: StatsSource,
        summary: StatsSummary,
    },
}

/// What a loaded Local stats tab was computed from.
enum StatsSource {
    /// The app's own scan of the session files, re-aggregated per range.
    Files { data: Arc<StatsData>, offset: i64 },
    /// Gentle Shell's own aggregation for the range shown, with what it left
    /// out. `day_cost` when its days carry their cost (5.0.0 on).
    Shell {
        notes: Vec<&'static str>,
        day_cost: bool,
    },
}

/// Where Local stats come from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StatsMode {
    /// The tab's Shell is still probing: wait, bounded, before deciding.
    Undecided,
    /// Gentle Shell over `get_usage_stats` (every project, per range).
    Shell,
    /// The session files (also before the tab is first shown: nothing is
    /// read either way until then).
    Files,
}

/// Local stats read from Gentle Shell: one answer per range (switching back
/// re-asks nothing), the read in flight, and the local UTC offset the chart
/// days are counted in.
struct ShellStats {
    mode: StatsMode,
    pending: Option<(ReadTicket, StatsRange)>,
    answers: HashMap<StatsRange, UsageStats>,
    offset: Option<i64>,
}

/// Where the Subscription tab gets its data: the session's subscription RPC
/// slice, or the legacy `auth.json` + provider API path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum UsageMode {
    Rpc,
    Legacy,
}

/// The provider ids the subscription RPC wire speaks, mapped to the same
/// titles the legacy cards use; any other provider shows its own id.
fn rpc_provider_title(id: &str) -> SharedString {
    match id {
        "codex" | "openai-codex" => "ChatGPT · Codex".into(),
        "openai" => "OpenAI · ChatGPT".into(),
        "claude" | "anthropic" => "Anthropic · Claude".into(),
        "claude-bridge" => "Claude Code · bridge".into(),
        "nan" => "NaN Cloud".into(),
        _ => id.to_string().into(),
    }
}

fn rpc_window_to_core(window: &RpcUsageWindow) -> UsageWindow {
    UsageWindow {
        label: window.label.clone(),
        used_percent: window.used_percent,
        window_seconds: window.window_seconds,
        reset_at: match window.reset_at {
            UsageResetAt::At(ms) => Some(ms as i64),
            UsageResetAt::Null => None,
        },
        used: window.used,
        budget: window.budget,
    }
}

fn rpc_limit_to_core(limit: &RpcUsageLimit) -> UsageLimit {
    UsageLimit {
        name: limit.name.clone(),
        windows: limit.windows.iter().map(rpc_window_to_core).collect(),
        limit_reached: limit.limit_reached,
    }
}

/// The wire provider → core usage, reused by the Usage screen's cards and
/// the session's plan-limits chip (the fetched time falls back to `now`).
pub(crate) fn rpc_provider_to_core(provider: &RpcProviderUsage, now: i64) -> ProviderUsage {
    ProviderUsage {
        provider: provider.provider.clone(),
        plan: provider.plan.clone(),
        limits: provider.limits.iter().map(rpc_limit_to_core).collect(),
        fetched_at: provider.fetched_at.map_or(now, |ms| ms as i64),
    }
}

/// One provider usage entry as a card: the closed status vocabulary maps to
/// the existing card states, keeping last good data for `pending`/`stale` and
/// showing `failed`/`unavailable` honestly.
fn rpc_provider_card(provider: &RpcProviderUsage, now: i64) -> ProviderCard {
    let core = rpc_provider_to_core(provider, now);
    let state = match provider.status {
        UsageStatus::Ok if core.limits.is_empty() => {
            CardState::Note("This provider reported no rate-limit windows for this plan.".into())
        }
        UsageStatus::Ok => CardState::Ready(core),
        UsageStatus::Pending => CardState::Pending((!core.limits.is_empty()).then_some(core)),
        UsageStatus::Stale => CardState::Stale((!core.limits.is_empty()).then_some(core)),
        UsageStatus::Failed => {
            CardState::Failed("Usage could not be read for this provider.".into())
        }
        UsageStatus::Unavailable => CardState::Unavailable,
    };
    ProviderCard {
        id: provider.provider.clone(),
        title: rpc_provider_title(&provider.provider),
        state,
        refresh: provider.refresh,
    }
}

/// The Subscription tab's card list from an RPC subscription snapshot.
fn rpc_cards(providers: &[RpcProviderUsage], now: i64) -> Vec<ProviderCard> {
    providers
        .iter()
        .map(|p| rpc_provider_card(p, now))
        .collect()
}

/// What the Subscription tab should do when the owning session changes, given
/// its current mode and whether the snapshot says the legacy path is due.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum UsageAction {
    /// Keep reading through RPC: seed once, then render the snapshot.
    Rpc,
    /// The peer just confirmed it does not offer the methods: run the legacy
    /// `auth.json` path exactly once.
    SwitchLegacy,
    /// Already on the legacy path: it is self-contained, so do nothing more.
    StayLegacy,
}

fn usage_action(current: UsageMode, legacy: bool) -> UsageAction {
    match (current, legacy) {
        (UsageMode::Rpc, true) => UsageAction::SwitchLegacy,
        (UsageMode::Rpc, false) => UsageAction::Rpc,
        (UsageMode::Legacy, _) => UsageAction::StayLegacy,
    }
}

/// The rendered Usage state from an RPC snapshot, as plain values so the
/// decision (including the empty-state copy) is testable without a session.
fn rpc_usage_state(
    not_ready: bool,
    pending: bool,
    providers: &[RpcProviderUsage],
    error: Option<String>,
    now: i64,
) -> UsageState {
    if not_ready || (pending && providers.is_empty()) {
        UsageState::Loading
    } else if !providers.is_empty() {
        UsageState::Loaded {
            cards: rpc_cards(providers, now),
            others: Vec::new(),
            empty: UsageEmpty::Reported,
        }
    } else if let Some(error) = error {
        UsageState::Failed(UsageText::External(error))
    } else {
        UsageState::Loaded {
            cards: Vec::new(),
            others: Vec::new(),
            empty: UsageEmpty::Reported,
        }
    }
}

pub struct UsageView {
    tab: Tab,
    usage: UsageState,
    stats: StatsState,
    range: StatsRange,
    metric: Metric,
    /// The agent tab whose subscription RPC slice feeds the Subscription tab
    /// and whose Shell answers Local stats when it offers them; `None` when
    /// the window has no agent (the legacy paths run instead).
    session: Option<Entity<SessionView>>,
    /// How the Subscription tab currently reads its data.
    mode: UsageMode,
    /// Local stats from Gentle Shell, when that is their source.
    shell_stats: ShellStats,
    _session_sub: Option<Subscription>,
    _usage_task: Task<()>,
    _stats_task: Task<()>,
    /// Set when the view drops, so a load still running frees what it read.
    closed: Arc<AtomicBool>,
}

impl EventEmitter<UsageEvent> for UsageView {}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl UsageView {
    pub fn new(
        _window: &mut Window,
        cx: &mut Context<Self>,
        session: Option<Entity<SessionView>>,
    ) -> Self {
        let mut this = Self {
            tab: Tab::Usage,
            usage: UsageState::Loading,
            stats: StatsState::Idle,
            range: StatsRange::All,
            metric: Metric::Tokens,
            session: session.clone(),
            mode: UsageMode::Legacy,
            shell_stats: ShellStats {
                mode: StatsMode::Files,
                pending: None,
                answers: HashMap::new(),
                offset: None,
            },
            _session_sub: None,
            _usage_task: Task::ready(()),
            _stats_task: Task::ready(()),
            closed: Arc::default(),
        };
        if let Some(session) = &session {
            this._session_sub = Some(cx.observe(session, |this, _, cx| this.session_changed(cx)));
        }
        this.refresh_usage(cx);
        this
    }

    /// Decide where the Subscription tab reads its data: the owning session's
    /// subscription RPC slice, or the legacy `auth.json` path.
    fn usage_mode(&self, cx: &App) -> UsageMode {
        match &self.session {
            Some(session) => {
                if session.read(cx).subscription_snapshot().legacy {
                    UsageMode::Legacy
                } else {
                    UsageMode::Rpc
                }
            }
            None => UsageMode::Legacy,
        }
    }

    /// The Subscription tab opened (or a session's capabilities just became
    /// known): decide RPC vs legacy, then load once.
    fn refresh_usage(&mut self, cx: &mut Context<Self>) {
        self.mode = self.usage_mode(cx);
        if self.mode == UsageMode::Rpc {
            if let Some(session) = self.session.clone() {
                session.update(cx, |session, cx| session.subscription_seed(cx));
            }
            self.apply_rpc_snapshot(cx);
        } else {
            self.refresh_usage_legacy(cx);
        }
    }

    /// The refresh button: RPC refreshes over `refresh_subscription_usage`;
    /// the legacy path re-fetches directly.
    fn reload_usage(&mut self, cx: &mut Context<Self>) {
        self.mode = self.usage_mode(cx);
        if self.mode == UsageMode::Rpc {
            if let Some(session) = self.session.clone() {
                session.update(cx, |session, cx| session.subscription_refresh(cx));
            }
            self.apply_rpc_snapshot(cx);
        } else {
            self.refresh_usage_legacy(cx);
        }
    }

    /// The owning session's subscription slice changed: re-read the RPC
    /// snapshot. A server `unsupported_method` switches to the legacy path at
    /// once (running its refresh exactly once); every other RPC error stays on
    /// the RPC path. The legacy path never runs from here once it is active,
    /// so a turn never re-reads `auth.json`.
    fn session_changed(&mut self, cx: &mut Context<Self>) {
        self.shell_stats_changed(cx);
        let legacy = match &self.session {
            Some(session) => session.read(cx).subscription_snapshot().legacy,
            None => true,
        };
        match usage_action(self.mode, legacy) {
            UsageAction::SwitchLegacy => {
                self.mode = UsageMode::Legacy;
                self.refresh_usage_legacy(cx);
            }
            UsageAction::Rpc => {
                self.mode = UsageMode::Rpc;
                if let Some(session) = self.session.clone() {
                    // Capabilities may just have arrived; seed is idempotent.
                    session.update(cx, |session, cx| session.subscription_seed(cx));
                }
                self.apply_rpc_snapshot(cx);
            }
            UsageAction::StayLegacy => {}
        }
    }

    /// Render the session's subscription snapshot into `self.usage`. When the
    /// RPC path has providers, they win (last good data); a whole-request
    /// failure only replaces the cards when there is nothing else to show.
    fn apply_rpc_snapshot(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let snapshot = session.read(cx).subscription_snapshot();
        self.usage = rpc_usage_state(
            snapshot.not_ready,
            snapshot.pending,
            &snapshot.providers,
            snapshot.error.clone(),
            now_ms(),
        );
        cx.notify();
    }

    /// The legacy `auth.json` + provider API path, unchanged from before. It
    /// runs only when there is no agent session, or its capabilities are known
    /// not to offer the subscription methods.
    fn refresh_usage_legacy(&mut self, cx: &mut Context<Self>) {
        self.usage = UsageState::Loading;
        cx.notify();
        let auth_path = pi_rpc::resolve_homes().agent.join("auth.json");
        self._usage_task = cx.spawn(async move |this, cx| {
            let now = now_ms();
            let discovery = cx
                .background_executor()
                .spawn(async move { read_auth(&auth_path).map(|auth| discover(&auth, now)) })
                .await;
            let discovery = match discovery {
                Ok(discovery) => discovery,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.usage = UsageState::Failed(error);
                        cx.notify();
                    });
                    return;
                }
            };
            let mut pending = Vec::new();
            let mut cards = Vec::new();
            for (card, fetch) in discovery.cards {
                if let Some(fetch) = fetch {
                    let id = card.id.clone();
                    pending.push((
                        id,
                        cx.background_executor()
                            .spawn(async move { run_fetch(fetch, now) }),
                    ));
                }
                cards.push(card);
            }
            if this
                .update(cx, |this, cx| {
                    this.usage = UsageState::Loaded {
                        cards,
                        others: discovery.others,
                        empty: UsageEmpty::SignedIn,
                    };
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            for (id, task) in pending {
                let state = task.await;
                let updated = this.update(cx, |this, cx| {
                    if let UsageState::Loaded { cards, .. } = &mut this.usage
                        && let Some(card) = cards.iter_mut().find(|c| c.id == id)
                    {
                        card.state = state;
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    return;
                }
            }
        });
    }

    /// Load Local stats afresh (first shown, or the refresh button): from
    /// Gentle Shell when the active agent tab's Shell offers
    /// `get_usage_stats`, from the session files otherwise. A tab still
    /// probing its Shell is waited for, at most [`READ_DECIDE_WAIT`].
    fn refresh_stats(&mut self, cx: &mut Context<Self>) {
        self.shell_stats.pending = None;
        self.shell_stats.answers.clear();
        self.shell_stats.mode = self.stats_mode(cx);
        match self.shell_stats.mode {
            StatsMode::Files => self.refresh_stats_files(cx),
            StatsMode::Shell => self.ask_shell_stats(cx),
            StatsMode::Undecided => {
                self.stats = StatsState::Loading;
                self._stats_task = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(READ_DECIDE_WAIT).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.shell_stats.mode == StatsMode::Undecided {
                            this.shell_stats.mode = StatsMode::Files;
                            this.refresh_stats_files(cx);
                        }
                    });
                });
                cx.notify();
            }
        }
    }

    fn stats_mode(&self, cx: &App) -> StatsMode {
        let Some(session) = &self.session else {
            return StatsMode::Files;
        };
        match session.read(cx).read_support(DesktopMethod::GetUsageStats) {
            ReadSupport::Rpc => StatsMode::Shell,
            ReadSupport::Legacy => StatsMode::Files,
            ReadSupport::NotReady => StatsMode::Undecided,
        }
    }

    /// The tab changed: a probe that just answered decides where stats come
    /// from, and an answer to the read in flight is taken.
    fn shell_stats_changed(&mut self, cx: &mut Context<Self>) {
        match self.shell_stats.mode {
            StatsMode::Undecided => {
                let mode = self.stats_mode(cx);
                if mode == StatsMode::Undecided {
                    return;
                }
                self.shell_stats.mode = mode;
                if mode == StatsMode::Shell {
                    self.ask_shell_stats(cx);
                } else {
                    self.refresh_stats_files(cx);
                }
            }
            StatsMode::Shell => self.take_shell_stats(cx),
            StatsMode::Files => {}
        }
    }

    /// Show the range from Gentle Shell: its answer when already read, or
    /// one `get_usage_stats` for every project. One read at a time: a range
    /// picked while another is in flight is asked once that one lands. A
    /// read the tab cannot queue falls back to the session files.
    fn ask_shell_stats(&mut self, cx: &mut Context<Self>) {
        if self.shell_stats.answers.contains_key(&self.range) {
            self.show_shell_stats(cx);
            return;
        }
        self.stats = StatsState::Loading;
        cx.notify();
        if self.shell_stats.pending.is_some() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return self.stats_fallback(cx);
        };
        let range = self.range;
        let request = ReadRequest::UsageStats {
            range: shell_stats::rpc_range(range),
            scope: StatsScope::All,
        };
        match session.update(cx, |session, cx| session.read(request, cx)) {
            Ok(ticket) => self.shell_stats.pending = Some((ticket, range)),
            Err(_) => self.stats_fallback(cx),
        }
    }

    /// The answer to the read in flight, if it arrived. Any failure (a Shell
    /// that refused or stopped answering) falls back to the session files.
    fn take_shell_stats(&mut self, cx: &mut Context<Self>) {
        let (Some((ticket, range)), Some(session)) =
            (self.shell_stats.pending, self.session.clone())
        else {
            return;
        };
        let Some(outcome) = session.update(cx, |session, _| session.take_read(ticket)) else {
            return;
        };
        self.shell_stats.pending = None;
        match outcome {
            ReadOutcome::UsageStats(stats) => {
                self.shell_stats.answers.insert(range, stats);
                self.ask_shell_stats(cx);
            }
            _ => self.stats_fallback(cx),
        }
    }

    fn stats_fallback(&mut self, cx: &mut Context<Self>) {
        self.shell_stats.mode = StatsMode::Files;
        self.shell_stats.pending = None;
        self.refresh_stats_files(cx);
    }

    /// Map the shown range's answer onto the summary, off the UI thread (the
    /// local offset is read once, from `date`).
    fn show_shell_stats(&mut self, cx: &mut Context<Self>) {
        let range = self.range;
        let Some(stats) = self.shell_stats.answers.get(&range).cloned() else {
            return;
        };
        let offset = self.shell_stats.offset;
        self._stats_task = cx.spawn(async move |this, cx| {
            let (summary, notes, day_cost, offset) = cx
                .background_executor()
                .spawn(async move {
                    let offset = offset.unwrap_or_else(local_offset_secs);
                    let summary = shell_stats::shell_summary(&stats, range, now_ms(), offset);
                    let day_cost = shell_stats::has_day_cost(&stats);
                    (summary, shell_stats::shell_notes(&stats), day_cost, offset)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.shell_stats.offset = Some(offset);
                if this.range != range || this.shell_stats.mode != StatsMode::Shell {
                    return;
                }
                this.stats = StatsState::Loaded {
                    source: StatsSource::Shell { notes, day_cost },
                    summary,
                };
                // An older Shell's days carry no cost: the chart shows tokens.
                if !day_cost {
                    this.metric = Metric::Tokens;
                }
                cx.notify();
            });
        });
    }

    fn refresh_stats_files(&mut self, cx: &mut Context<Self>) {
        self.stats = StatsState::Loading;
        cx.notify();
        let root = pi_rpc::resolve_homes().agent.join("sessions");
        let range = self.range;
        let closed = self.closed.clone();
        self._stats_task = cx.spawn(async move |this, cx| {
            let (data, offset, summary) = cx
                .background_executor()
                .spawn(async move {
                    let offset = local_offset_secs();
                    let data = load_sessions(&root, MAX_SESSION_BYTES);
                    if closed.load(Ordering::SeqCst) {
                        forget_sessions();
                    }
                    let summary = aggregate_stats(&data.sessions, range, now_ms(), offset);
                    (Arc::new(data), offset, summary)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                // The range may have changed while loading; re-aggregate if so.
                let summary = if this.range == range {
                    summary
                } else {
                    aggregate_stats(&data.sessions, this.range, now_ms(), offset)
                };
                this.stats = StatsState::Loaded {
                    source: StatsSource::Files { data, offset },
                    summary,
                };
                cx.notify();
            });
        });
    }

    /// Switch to the Local stats tab (the `/gentle:stats` command).
    pub fn show_stats(&mut self, cx: &mut Context<Self>) {
        self.set_tab(Tab::Stats, cx);
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::Stats && matches!(self.stats, StatsState::Idle) {
            self.refresh_stats(cx);
        }
        cx.notify();
    }

    fn set_range(&mut self, range: StatsRange, cx: &mut Context<Self>) {
        self.range = range;
        match &mut self.stats {
            StatsState::Loaded {
                source: StatsSource::Files { data, offset },
                summary,
            } => *summary = aggregate_stats(&data.sessions, range, now_ms(), *offset),
            StatsState::Loaded { .. } | StatsState::Loading
                if self.shell_stats.mode == StatsMode::Shell =>
            {
                self.ask_shell_stats(cx)
            }
            _ => {}
        }
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        match self.tab {
            Tab::Usage => self.reload_usage(cx),
            Tab::Stats => self.refresh_stats(cx),
        }
    }
}

/// The parsed sessions are only worth keeping while Usage is open.
impl Drop for UsageView {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        forget_sessions();
    }
}

#[cfg(test)]
mod tests {
    use pi_rpc::desktop_rpc::{UsageRefresh, UsageStatus};

    use super::{
        UsageAction, UsageEmpty, UsageMode, UsageState, rpc_cards, rpc_provider_card,
        rpc_provider_title, rpc_usage_state, usage_action,
    };
    use crate::features::usage::credentials::CardState;

    #[test]
    fn t14_usage_path_error_keeps_external_details_verbatim() {
        use crate::shared::i18n::with_test_language;
        use gentle_core::i18n::Language;

        let text = super::UsageText::PathError("log-{error}".into(), "cannot open {path}".into());
        let actual = with_test_language(Language::English, || text.render());
        assert_eq!(actual, "Could not read log-{error}: cannot open {path}");
    }

    const NOW: i64 = 1_762_400_000_000;

    /// Decode one provider through the adapter's own parser, so the mapping
    /// tests stay on the real wire shape.
    fn provider(
        status: UsageStatus,
        limits: serde_json::Value,
    ) -> pi_rpc::desktop_rpc::ProviderUsage {
        usage_with(
            serde_json::json!({ "provider": "codex", "status": status.as_str(), "limits": limits }),
        )
        .providers
        .into_iter()
        .next()
        .unwrap()
    }

    fn usage_with(provider: serde_json::Value) -> pi_rpc::desktop_rpc::SubscriptionUsage {
        let line = serde_json::json!({
            "schema": pi_rpc::desktop_rpc::SCHEMA,
            "kind": "response",
            "requestId": "desktop-1",
            "ok": true,
            "data": {"providers": [provider]},
        })
        .to_string();
        match pi_rpc::desktop_rpc::decode_payload(&line).unwrap() {
            pi_rpc::desktop_rpc::DesktopRecord::Response(response) => {
                response.subscription_usage().unwrap()
            }
            other => panic!("expected a response, got {other:?}"),
        }
    }

    #[test]
    fn rpc_provider_titles_map_the_wire_vocabulary() {
        assert_eq!(rpc_provider_title("codex"), "ChatGPT · Codex");
        assert_eq!(rpc_provider_title("openai-codex"), "ChatGPT · Codex");
        assert_eq!(rpc_provider_title("openai"), "OpenAI · ChatGPT");
        assert_eq!(rpc_provider_title("claude"), "Anthropic · Claude");
        assert_eq!(rpc_provider_title("anthropic"), "Anthropic · Claude");
        // Claude through the bridge keeps a title of its own: the windows are
        // the same Claude plan, but the card says which path served them.
        assert_eq!(rpc_provider_title("claude-bridge"), "Claude Code · bridge");
        assert_eq!(rpc_provider_title("nan"), "NaN Cloud");
        // A provider the app has no title for shows its own id, not a blank
        // "Provider" (DeepSeek, Copilot, …).
        assert_eq!(rpc_provider_title("deepseek"), "deepseek");
    }

    #[test]
    fn every_status_maps_to_a_card_state() {
        let window = serde_json::json!([{
            "label": "5h", "usedPercent": 42.5, "windowSeconds": 18000, "resetAt": null,
        }]);
        let limits =
            serde_json::json!([{ "name": "prompts", "limitReached": false, "windows": window }]);

        let ok = rpc_provider_card(&provider(UsageStatus::Ok, limits.clone()), NOW);
        assert!(matches!(ok.state, CardState::Ready(_)));
        assert_eq!(ok.title, "ChatGPT · Codex");

        let ok_empty = rpc_provider_card(&provider(UsageStatus::Ok, serde_json::json!([])), NOW);
        assert!(matches!(ok_empty.state, CardState::Note(_)));

        let pending = rpc_provider_card(&provider(UsageStatus::Pending, limits.clone()), NOW);
        assert!(matches!(&pending.state, CardState::Pending(Some(_))));

        let pending_empty =
            rpc_provider_card(&provider(UsageStatus::Pending, serde_json::json!([])), NOW);
        assert!(matches!(pending_empty.state, CardState::Pending(None)));

        let stale = rpc_provider_card(&provider(UsageStatus::Stale, limits), NOW);
        assert!(matches!(&stale.state, CardState::Stale(Some(_))));

        let failed = rpc_provider_card(&provider(UsageStatus::Failed, serde_json::json!([])), NOW);
        assert!(matches!(failed.state, CardState::Failed(_)));

        let unavailable = rpc_provider_card(
            &provider(UsageStatus::Unavailable, serde_json::json!([])),
            NOW,
        );
        assert!(matches!(unavailable.state, CardState::Unavailable));
    }

    #[test]
    fn claude_windows_map_with_their_labels_and_resets() {
        let usage = usage_with(serde_json::json!({
            "provider": "claude",
            "status": "ok",
            "fetchedAt": 1788600000000u64,
            "limits": [{
                "name": "prompts",
                "limitReached": false,
                "windows": [
                    {"label": "5h", "usedPercent": 42.5, "windowSeconds": 18000, "resetAt": null},
                    {"label": "week", "usedPercent": 10.0, "windowSeconds": 604800, "resetAt": 1789200000000u64},
                ],
            }],
        }));
        let cards = rpc_cards(&usage.providers, NOW);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Anthropic · Claude");
        let CardState::Ready(core) = &cards[0].state else {
            panic!("expected a ready card");
        };
        assert_eq!(core.limits.len(), 1);
        assert_eq!(core.limits[0].windows[0].label, "5h");
        assert_eq!(core.limits[0].windows[0].reset_at, None);
        assert_eq!(core.limits[0].windows[1].label, "week");
        assert_eq!(core.limits[0].windows[1].reset_at, Some(1789200000000));
        assert_eq!(core.fetched_at, 1788600000000);
    }

    #[test]
    fn refresh_dispositions_are_preserved_on_the_card() {
        let bare = rpc_provider_card(&provider(UsageStatus::Ok, serde_json::json!([])), NOW);
        assert_eq!(bare.refresh, None);

        // A provider with a refresh disposition keeps it for the renderer.
        let usage = usage_with(serde_json::json!({
            "provider": "nan", "status": "ok", "refresh": "out_of_scope", "limits": [],
        }));
        let cards = rpc_cards(&usage.providers, NOW);
        assert_eq!(cards[0].refresh, Some(UsageRefresh::OutOfScope));
    }

    #[test]
    fn rpc_empty_state_uses_reported_copy_not_sign_in() {
        let state = rpc_usage_state(false, false, &[], None, NOW);
        match state {
            UsageState::Loaded { cards, empty, .. } => {
                assert!(cards.is_empty());
                assert_eq!(empty, UsageEmpty::Reported);
                assert!(empty.message().contains("has not reported"));
                assert!(!empty.message().contains("signed in"));
            }
            other => panic!("expected a loaded empty state, got {other:?}"),
        }

        // Loading and failure states are unchanged.
        assert!(matches!(
            rpc_usage_state(true, false, &[], None, NOW),
            UsageState::Loading
        ));
        assert!(matches!(
            rpc_usage_state(false, true, &[], None, NOW),
            UsageState::Loading
        ));
        assert!(matches!(
            rpc_usage_state(false, false, &[], Some("boom".into()), NOW),
            UsageState::Failed(_)
        ));
    }

    #[test]
    fn empty_copy_keeps_the_legacy_sign_in_only_on_the_legacy_path() {
        let sign_in = UsageEmpty::SignedIn.message();
        assert!(sign_in.contains("signed in"));
        assert!(sign_in.contains("Sign in"));

        let reported = UsageEmpty::Reported.message();
        assert_ne!(reported, sign_in);
        assert!(!reported.contains("signed in"));
        assert!(!reported.contains("Sign in"));
    }

    #[test]
    fn usage_action_switches_to_legacy_only_once_and_only_on_unsupported() {
        assert_eq!(usage_action(UsageMode::Rpc, false), UsageAction::Rpc);
        assert_eq!(
            usage_action(UsageMode::Rpc, true),
            UsageAction::SwitchLegacy
        );
        // Once on the legacy path it is self-contained: no second legacy load.
        assert_eq!(
            usage_action(UsageMode::Legacy, true),
            UsageAction::StayLegacy
        );
        assert_eq!(
            usage_action(UsageMode::Legacy, false),
            UsageAction::StayLegacy
        );
    }
}
