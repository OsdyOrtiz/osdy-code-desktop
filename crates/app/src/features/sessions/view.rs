//! Browser state and actions: construction, choosing who lists the sessions,
//! background reload, scope switch, search filtering and opening a session.

use std::path::{Path, PathBuf};

use gentle_core::sessions::{SessionSummary, listed_matches, project_name};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use pi_rpc::desktop_rpc::{
    DesktopErrorCode, DesktopMethod, ListSessionsQuery, MAX_SESSIONS_LIMIT, SessionEntry,
    SessionsScope,
};

use super::discovery::{find_session, list_sessions};
use super::{
    FILTER_PLACEHOLDER, OpenError, Pending, Scope, SessionsEvent, SessionsView, Source, t,
};
use crate::features::session::{
    READ_DECIDE_WAIT, ReadFailure, ReadOutcome, ReadRequest, ReadSupport, SessionView,
};

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((dunce::canonicalize(a), dunce::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// The paths in `sessions` that are the file `current`, also when spelled
/// differently (a symlinked folder, a `..`). Blocking (it resolves every
/// path): call off the UI thread.
pub(super) fn current_rows(current: Option<&Path>, sessions: &[SessionSummary]) -> Vec<PathBuf> {
    let Some(current) = current else {
        return Vec::new();
    };
    sessions
        .iter()
        .filter(|s| same_file(current, &s.path))
        .map(|s| s.path.clone())
        .collect()
}

/// One listed session, from either source.
#[derive(Clone, Copy)]
pub(super) enum Row<'a> {
    File(&'a SessionSummary),
    Shell(&'a SessionEntry),
}

impl Row<'_> {
    /// The session to open, owned (a click handler outlives the list).
    pub(super) fn target(self) -> Target {
        match self {
            Row::File(session) => Target::File(Box::new(session.clone())),
            Row::Shell(entry) => Target::Shell(Box::new(entry.clone())),
        }
    }
}

/// A session to open, from either source.
pub(super) enum Target {
    File(Box<SessionSummary>),
    Shell(Box<SessionEntry>),
}

/// Who should list the sessions of `cwd`, given the agent tab (if any):
/// its Shell only when it browses the same folder (Gentle Shell's project
/// is the tab's) and offers both methods; the files when it cannot.
pub(super) fn choose_source(
    tab_cwd: Option<&Path>,
    cwd: &Path,
    support: [ReadSupport; 2],
) -> Source {
    if tab_cwd != Some(cwd) || support.contains(&ReadSupport::Legacy) {
        Source::Files
    } else if support.contains(&ReadSupport::NotReady) {
        Source::Undecided
    } else {
        Source::Shell
    }
}

/// Where a switch to `entry` is resolved, or `None` when it belongs to
/// another project and opens in a new tab instead. In "This project" the
/// server's default (the project) applies; in "All" a session named after
/// another project is known to be elsewhere, and one with this project's
/// name is tried here first (another folder may share the name).
pub(super) fn switch_scope(
    scope: Scope,
    entry: &SessionEntry,
    cwd: &Path,
) -> Option<Option<SessionsScope>> {
    match scope {
        Scope::Project => Some(None),
        Scope::All if entry.project == project_name(cwd) => Some(Some(SessionsScope::Project)),
        Scope::All => None,
    }
}

/// Whether a refused switch means the session is another project's: not in
/// this project (`not_found` where only the project was searched), or a
/// Shell that turned out not to switch by id at all.
pub(super) fn opens_elsewhere(failure: &ReadFailure, scope: Option<SessionsScope>) -> bool {
    match failure {
        ReadFailure::Unsupported | ReadFailure::NotOffered => true,
        _ => {
            failure.code() == Some(DesktopErrorCode::NotFound)
                && scope == Some(SessionsScope::Project)
        }
    }
}

impl SessionsView {
    pub fn new(
        cwd: PathBuf,
        current_session_file: Option<PathBuf>,
        session: Option<Entity<SessionView>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::empty(cwd, current_session_file, session, window, cx);
        this.source = this.decide(cx);
        this.reload(cx);
        this
    }

    /// The screen with nothing listed or asked yet.
    pub(super) fn empty(
        cwd: PathBuf,
        current_session_file: Option<PathBuf>,
        session: Option<Entity<SessionView>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(t(FILTER_PLACEHOLDER)));
        let mut subscriptions = vec![cx.subscribe_in(
            &filter,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.open_first(cx);
                }
                cx.notify();
            },
        )];
        if let Some(session) = &session {
            subscriptions.push(cx.observe(session, |this, _, cx| this.session_changed(cx)));
        }
        Self {
            cwd,
            current: current_session_file,
            scope: Scope::Project,
            session,
            source: Source::Files,
            sessions: Vec::new(),
            current_rows: Vec::new(),
            entries: Vec::new(),
            next_cursor: None,
            pending: None,
            error: None,
            loading: false,
            filter,
            filter_language: crate::shared::i18n::language(),
            _load: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn decide(&self, cx: &App) -> Source {
        let Some(session) = &self.session else {
            return Source::Files;
        };
        let session = session.read(cx);
        choose_source(
            Some(session.cwd().as_path()),
            &self.cwd,
            [DesktopMethod::ListSessions, DesktopMethod::SwitchSession]
                .map(|method| session.read_support(method)),
        )
    }

    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        match self.source {
            Source::Files => self.reload_files(cx),
            Source::Shell => self.list_page(false, cx),
            Source::Undecided => {
                self.loading = true;
                self._load = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(READ_DECIDE_WAIT).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.source == Source::Undecided {
                            this.use_files(cx);
                        }
                    });
                });
                cx.notify();
            }
        }
    }

    /// The agent tab changed: a probe that just answered decides who lists,
    /// and Gentle Shell's answer to the read in flight is taken.
    fn session_changed(&mut self, cx: &mut Context<Self>) {
        match self.source {
            Source::Undecided => {
                let source = self.decide(cx);
                if source != Source::Undecided {
                    self.source = source;
                    self.reload(cx);
                }
            }
            Source::Shell => self.take_answer(cx),
            Source::Files => {}
        }
    }

    /// From here on the app reads the files: the tab's Shell cannot list.
    fn use_files(&mut self, cx: &mut Context<Self>) {
        self.source = Source::Files;
        self.pending = None;
        self.entries.clear();
        self.next_cursor = None;
        self.reload_files(cx);
    }

    fn reload_files(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let agent = pi_rpc::resolve_homes().agent;
        let cwd = self.cwd.clone();
        let scope = self.scope;
        let current = self.current.clone();
        // Replacing the task drops (cancels) any load still in flight.
        self._load = cx.spawn(async move |this, cx| {
            let (sessions, current_rows) = cx
                .background_executor()
                .spawn(async move {
                    let sessions = list_sessions(&agent, &cwd, scope);
                    let current_rows = current_rows(current.as_deref(), &sessions);
                    (sessions, current_rows)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.sessions = sessions;
                this.current_rows = current_rows;
                this.loading = false;
                cx.notify();
            });
        });
        cx.notify();
    }

    /// Ask Gentle Shell for the first page of the scope, or (`more`) the page
    /// after the ones shown.
    pub(super) fn list_page(&mut self, more: bool, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return self.use_files(cx);
        };
        if matches!(self.pending, Some(Pending::Switch { .. })) {
            return;
        }
        let cursor = match (more, &self.next_cursor) {
            (false, _) => None,
            (true, Some(cursor)) => Some(cursor.clone()),
            (true, None) => return,
        };
        let query = ListSessionsQuery {
            scope: Some(self.scope.wire()),
            limit: Some(MAX_SESSIONS_LIMIT),
            cursor,
        };
        match session.update(cx, |session, cx| {
            session.read(ReadRequest::ListSessions(query), cx)
        }) {
            Ok(ticket) => {
                self.pending = Some(Pending::List {
                    ticket,
                    scope: self.scope,
                    more,
                });
                self.loading = true;
                cx.notify();
            }
            Err(_) => self.use_files(cx),
        }
    }

    /// Gentle Shell's answer to the read in flight, once it arrived. A page
    /// that cannot be read hands the list to the files; a switch that did
    /// not happen says why, or opens the session in a new tab when it is
    /// another project's.
    fn take_answer(&mut self, cx: &mut Context<Self>) {
        let (Some(session), Some(pending)) = (self.session.clone(), self.pending.as_ref()) else {
            return;
        };
        let ticket = match pending {
            Pending::List { ticket, .. } | Pending::Switch { ticket, .. } => *ticket,
        };
        let Some(outcome) = session.update(cx, |session, _| session.take_read(ticket)) else {
            return;
        };
        match (self.pending.take(), outcome) {
            (Some(Pending::List { scope, more, .. }), ReadOutcome::Sessions(list)) => {
                if scope == self.scope {
                    if !more {
                        self.entries.clear();
                    }
                    self.entries.extend(list.sessions);
                    self.next_cursor = list.next_cursor;
                }
                self.loading = false;
            }
            (Some(Pending::List { .. }), _) => self.use_files(cx),
            (Some(Pending::Switch { .. }), ReadOutcome::Switched(result)) => {
                if result.cancelled {
                    self.error = Some(OpenError::Cancelled);
                } else {
                    // The tab replaced its session (or already ran it).
                    cx.emit(SessionsEvent::Close);
                }
            }
            (Some(Pending::Switch { entry, scope, .. }), ReadOutcome::Failed(failure)) => {
                if opens_elsewhere(&failure, scope) {
                    self.open_from_file(entry, cx);
                } else {
                    self.error = Some(OpenError::Request(failure));
                }
            }
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.scope = scope;
            self.sessions.clear();
            self.entries.clear();
            self.next_cursor = None;
            self.reload(cx);
        }
    }

    /// Whether `row` is the tab's session; drawn per row, so it compares
    /// paths resolved when the list loaded.
    pub(super) fn is_current(&self, row: Row) -> bool {
        match row {
            Row::File(session) => self.current_rows.contains(&session.path),
            Row::Shell(entry) => entry.current,
        }
    }

    /// Whether a switch to `entry` is in flight.
    pub(super) fn is_opening(&self, entry: &SessionEntry) -> bool {
        matches!(&self.pending, Some(Pending::Switch { entry: e, .. }) if e.id == entry.id)
    }

    /// How many sessions are listed (loaded pages, for Gentle Shell).
    pub(super) fn count(&self) -> usize {
        match self.source {
            Source::Shell => self.entries.len(),
            Source::Undecided | Source::Files => self.sessions.len(),
        }
    }

    /// Whether Gentle Shell has older sessions than the pages shown.
    pub(super) fn has_more(&self) -> bool {
        self.source == Source::Shell && self.next_cursor.is_some()
    }

    pub(super) fn visible(&self, cx: &App) -> Vec<Row<'_>> {
        let query = self.filter.read(cx).value().trim().to_lowercase();
        match self.source {
            Source::Shell => self
                .entries
                .iter()
                .filter(|e| listed_matches(&query, e.name.as_deref(), &e.first_message, &e.project))
                .map(Row::Shell)
                .collect(),
            Source::Undecided | Source::Files => self
                .sessions
                .iter()
                .filter(|s| s.matches(&query))
                .map(Row::File)
                .collect(),
        }
    }

    /// What Enter in the filter does: open the first visible match.
    pub(super) fn open_first(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.visible(cx).first().map(|row| row.target()) {
            self.open_target(&target, cx);
        }
    }

    pub(super) fn open_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        match target {
            Target::File(session) => self.open(session, cx),
            Target::Shell(entry) => self.open_entry(entry, cx),
        }
    }

    pub(super) fn open(&mut self, session: &SessionSummary, cx: &mut Context<Self>) {
        // Not `is_current`: a session found by id may not be listed.
        if self
            .current
            .as_deref()
            .is_some_and(|c| same_file(c, &session.path))
        {
            cx.emit(SessionsEvent::Close);
            return;
        }
        let cwd = if session.cwd.as_os_str().is_empty() {
            self.cwd.clone()
        } else {
            session.cwd.clone()
        };
        cx.emit(SessionsEvent::Open {
            path: session.path.clone(),
            cwd,
        });
    }

    /// Open a session Gentle Shell listed: `switch_session` by id in the
    /// agent tab, or a new tab when it is another project's.
    pub(super) fn open_entry(&mut self, entry: &SessionEntry, cx: &mut Context<Self>) {
        if entry.current {
            cx.emit(SessionsEvent::Close);
            return;
        }
        if matches!(self.pending, Some(Pending::Switch { .. })) {
            return;
        }
        self.error = None;
        let (Some(scope), Some(session)) = (
            switch_scope(self.scope, entry, &self.cwd),
            self.session.clone(),
        ) else {
            return self.open_from_file(entry.clone(), cx);
        };
        let request = ReadRequest::SwitchSession {
            id: entry.id.clone(),
            scope,
        };
        match session.update(cx, |session, cx| session.read(request, cx)) {
            Ok(ticket) => {
                // A page still loading is not worth waiting for: the screen
                // closes once the switch lands.
                self.loading = false;
                self.pending = Some(Pending::Switch {
                    ticket,
                    entry: entry.clone(),
                    scope,
                });
            }
            Err(_) => self.open_from_file(entry.clone(), cx),
        }
        cx.notify();
    }

    /// Find `entry`'s file by its id and open it the files' way: in a new tab
    /// rooted at its folder (or, for this folder, Pi's `switch_session
    /// {sessionPath}`).
    fn open_from_file(&mut self, entry: SessionEntry, cx: &mut Context<Self>) {
        let agent = pi_rpc::resolve_homes().agent;
        let id = entry.id;
        self._load = cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { find_session(&agent, &id) })
                .await;
            let _ = this.update(cx, |this, cx| match found {
                Some(session) => this.open(&session, cx),
                None => {
                    this.error = Some(OpenError::MissingFile);
                    cx.notify();
                }
            });
        });
    }
}
