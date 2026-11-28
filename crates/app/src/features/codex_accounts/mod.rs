//! Account picker for the display-only osdy-gentle widget.

use std::time::Instant;

use gentle_core::codex_accounts::{Feed, USAGE_TIMEOUT, UsageRequest};
use gentle_core::transcript::NoticeLevel;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::session::{SessionEvent, SessionView};
use crate::shared::{i18n::t, ui};

pub(super) fn button(feed: &Feed) -> Button {
    let label = feed.active.as_deref().unwrap_or(t("Codex"));
    Button::new("codex-accounts-button")
        .ghost()
        .xsmall()
        .max_w(rems(148. / 16.))
        .text_color(ui::muted())
        .icon(Icon::new(IconName::KeyRound))
        .label(label.to_owned())
        .tooltip(t("Codex accounts"))
}

pub(super) enum PickerEvent {
    Close,
    Command(String),
    ViewUsage(String),
    InvalidName,
}

pub(super) struct AccountsView {
    feed: Feed,
    usage_available: bool,
    pending_usage: Vec<(UsageRequest, Instant)>,
    _owner_subscriptions: [Subscription; 2],
    selected: usize,
    focus: FocusHandle,
    scroll: ScrollHandle,
}

impl EventEmitter<PickerEvent> for AccountsView {}

impl AccountsView {
    pub(super) fn new(
        feed: Feed,
        owner: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let usage_available = owner.read(cx).has_codex_usage();
        let subscriptions = [
            cx.observe(owner, |this, owner, cx| {
                let owner = owner.read(cx);
                this.sync_owner(owner.codex_account_feed(), owner.has_codex_usage(), cx);
            }),
            cx.subscribe(owner, |this, _, event, cx| {
                // Extension notices carry no command/row ID. A warning or
                // error from this tab ends its outstanding usage indicators.
                if matches!(
                    event,
                    SessionEvent::Notice {
                        level: NoticeLevel::Warning | NoticeLevel::Error,
                        ..
                    }
                ) {
                    this.finish_usage(true, cx);
                }
            }),
        ];
        Self {
            feed,
            usage_available,
            pending_usage: Vec::new(),
            _owner_subscriptions: subscriptions,
            selected: 0,
            focus,
            scroll: ScrollHandle::new(),
        }
    }

    fn sync_owner(&mut self, feed: Option<Feed>, registered: bool, cx: &mut Context<Self>) {
        let available = feed.is_some() && registered;
        if self.usage_available != available {
            self.usage_available = available;
            cx.notify();
        }
        if let Some(feed) = feed.filter(|feed| *feed != self.feed) {
            self.feed = feed;
            self.selected = self.selected.min(self.feed.accounts.len());
            cx.notify();
        }
        self.finish_usage(!available, cx);
    }

    fn finish_usage(&mut self, failed: bool, cx: &mut Context<Self>) {
        let before = self.pending_usage.len();
        self.pending_usage
            .retain(|(request, start)| !request.finished(&self.feed, failed, start.elapsed()));
        if self.pending_usage.len() != before {
            cx.notify();
        }
    }

    fn view_usage(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(account) = self.feed.accounts.get(index) else {
            return;
        };
        if !self.usage_available
            || self
                .pending_usage
                .iter()
                .any(|(request, _)| request.matches(account))
        {
            return;
        }
        let Some(request) = UsageRequest::new(account) else {
            return;
        };
        let command = request.command();
        self.pending_usage.push((request, Instant::now()));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(USAGE_TIMEOUT).await;
            let _ = this.update(cx, |this, cx| this.finish_usage(false, cx));
        })
        .detach();
        cx.emit(PickerEvent::ViewUsage(command));
        cx.notify();
    }

    fn activate(&self, index: usize, cx: &mut Context<Self>) {
        let event = match self.feed.accounts.get(index) {
            Some(account) if account.active => PickerEvent::Close,
            Some(account) => match account.command_name() {
                Some(name) => PickerEvent::Command(format!("/osdy-profile {name}")),
                None => PickerEvent::InvalidName,
            },
            None => PickerEvent::Command("/osdy-account".into()),
        };
        cx.emit(event);
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let manage = self.feed.accounts.len();
        match event.keystroke.key.as_str() {
            "up" => self.selected = self.selected.saturating_sub(1),
            "down" => self.selected = (self.selected + 1).min(manage),
            "enter" => {
                self.activate(self.selected, cx);
                cx.stop_propagation();
                return;
            }
            "u" if self.usage_available && event.keystroke.modifiers == Modifiers::default() => {
                self.view_usage(self.selected, cx);
                cx.stop_propagation();
                return;
            }
            // Esc is the Workspace's standard CloseOverlay action.
            _ => return,
        }
        if self.selected < manage {
            self.scroll.scroll_to_item(self.selected);
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for AccountsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (backdrop, panel) = ui::screen(Some(640.));
        let manage = self.feed.accounts.len();
        backdrop.occlude().child(ui::pop_in(
            "codex-accounts-screen",
            panel
                .w_full()
                .max_w(rems(640. / 16.))
                .track_focus(&self.focus)
                .on_key_down(cx.listener(Self::key_down))
                .child(
                    ui::screen_header()
                        .child(
                            Icon::new(IconName::KeyRound)
                                .small()
                                .text_color(ui::muted()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t("Codex accounts")),
                        )
                        .child(
                            Button::new("codex-accounts-close")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(IconName::X))
                                .tooltip(t("Close"))
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Close))),
                        ),
                )
                .child(
                    v_flex()
                        .id("codex-accounts-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .p_4()
                        .gap_1()
                        .when(manage == 0, |body| {
                            body.child(
                                div()
                                    .py_8()
                                    .text_center()
                                    .text_sm()
                                    .text_color(ui::muted())
                                    .child(t("No saved Codex accounts.")),
                            )
                        })
                        .children(
                            self.feed
                                .accounts
                                .iter()
                                .enumerate()
                                .map(|(index, account)| {
                                    let loading = self
                                        .pending_usage
                                        .iter()
                                        .any(|(request, _)| request.matches(account));
                                    let select = h_flex()
                                        .id(("codex-account", index))
                                        .flex_1()
                                        .min_w_0()
                                        .gap_2()
                                        .py_2()
                                        .cursor_pointer()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_sm()
                                                .text_color(ui::fg())
                                                .child(account.name.clone()),
                                        )
                                        .when(account.active, |row| {
                                            row.child(ui::chip(t("Active"), ui::success()))
                                        })
                                        .when_some(account.binding.clone(), |row, binding| {
                                            row.child(
                                                ui::chip(binding, ui::info())
                                                    .max_w(rems(128. / 16.))
                                                    .truncate(),
                                            )
                                        })
                                        .when_some(account.quota.clone(), |row, quota| {
                                            row.child(
                                                div()
                                                    .max_w(rems(160. / 16.))
                                                    .truncate()
                                                    .text_xs()
                                                    .text_color(ui::muted())
                                                    .child(quota),
                                            )
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.activate(index, cx)
                                        }));
                                    h_flex()
                                        .flex_shrink_0()
                                        .gap_2()
                                        .rounded_md()
                                        .px_3()
                                        .when(self.selected == index, |row| {
                                            row.bg(ui::surface_hover())
                                        })
                                        .hover(|row| row.bg(ui::surface_hover()))
                                        .child(select)
                                        .when(
                                            self.usage_available
                                                && account.command_name().is_some(),
                                            |row| {
                                                row.child(
                                                    Button::new(("codex-view-usage", index))
                                                        .ghost()
                                                        .xsmall()
                                                        .icon(Icon::new(IconName::Gauge))
                                                        .tooltip(t(if loading {
                                                            "Loading usage…"
                                                        } else {
                                                            "View usage"
                                                        }))
                                                        .loading(loading)
                                                        .disabled(loading)
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.focus.focus(window, cx);
                                                                this.view_usage(index, cx);
                                                            },
                                                        )),
                                                )
                                            },
                                        )
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(ui::border())
                        .gap_2()
                        .child(
                            Button::new("codex-accounts-manage")
                                .ghost()
                                .small()
                                .selected(self.selected == manage)
                                .label(t("Manage accounts…"))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.activate(manage, cx)),
                                ),
                        )
                        .child(div().flex_1())
                        .when(self.usage_available && manage > 0, |footer| {
                            footer
                                .child(ui::key_button("codex-usage-key", "u").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.view_usage(this.selected, cx)
                                    }),
                                ))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ui::muted())
                                        .child(t("View usage")),
                                )
                        }),
                ),
        ))
    }
}
