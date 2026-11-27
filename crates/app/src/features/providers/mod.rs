//! Providers: sign in to model providers (OAuth or API key) without a terminal.
//! This file keeps the view's types and constructor; children add `impl`s.

use std::path::PathBuf;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use pi_rpc::{AuthPrompt, LoginHelper, ProviderInfo};

use crate::shared::i18n::{language, t};

mod flow_view;
mod list;
mod login;
mod render;

pub enum ProvidersEvent {
    Close,
    /// Credentials changed; sessions should reload their models.
    Changed,
    /// Agents run in a backend: sign in there, in a terminal in this folder
    /// with this command typed (`gentle-shell`, then `/login`).
    Terminal(PathBuf, String),
}

struct Flow {
    provider: String,
    log: Vec<String>,
    auth_url: Option<(String, Option<String>)>,
    device: Option<(String, String)>,
    prompt: Option<(u64, AuthPrompt)>,
    finished: Option<Result<(), String>>,
}

enum SignInError {
    HelperStopped,
    Detail(String),
}

pub struct ProvidersView {
    helper: Option<LoginHelper>,
    error: Option<SignInError>,
    providers: Vec<ProviderInfo>,
    filter: Entity<InputState>,
    filter_language: gentle_core::i18n::Language,
    answer: Entity<InputState>,
    secret: Entity<InputState>,
    flow: Option<Flow>,
    /// Where the folder's agents run, when not on this computer: they sign in
    /// there (`/login` in a terminal), not with this computer's helper.
    remote: Option<(PathBuf, String)>,
    _io: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProvidersEvent> for ProvidersView {}

impl ProvidersView {
    pub fn new(cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(t("Search providers")));
        let answer = cx.new(|cx| InputState::new(window, cx));
        let secret = cx.new(|cx| InputState::new(window, cx).masked(true));
        let submit_on_enter = |this: &mut Self,
                               _: &Entity<InputState>,
                               event: &InputEvent,
                               window: &mut Window,
                               cx: &mut Context<Self>| {
            if let InputEvent::PressEnter { .. } = event {
                this.submit_answer(window, cx);
            }
        };
        let subscriptions = vec![
            cx.subscribe_in(&secret, window, submit_on_enter),
            cx.subscribe_in(&filter, window, |_, _, _: &InputEvent, _, cx| cx.notify()),
            cx.subscribe_in(&answer, window, submit_on_enter),
        ];
        let mut this = Self {
            helper: None,
            error: None,
            providers: Vec::new(),
            filter,
            filter_language: language(),
            answer,
            secret,
            flow: None,
            remote: None,
            _io: Task::ready(()),
            _subscriptions: subscriptions,
        };
        match crate::features::backend::for_tab(&cwd) {
            gentle_core::backend::Backend::Local => this.start(cwd, cx),
            choice => this.remote = Some((cwd, choice.label())),
        }
        this
    }
}
