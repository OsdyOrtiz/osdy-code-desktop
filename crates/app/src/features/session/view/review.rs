//! The review-mode chip beside the permission chip, and the confirmation
//! Gentle Shell asks for before the switch changes. Only on a Shell that
//! offers `get_review_mode`/`set_review_mode` and has a review owner; older
//! ones show nothing here.

use crate::shared::i18n::t;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::{Popover, PopoverState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use pi_rpc::desktop_rpc::{ReviewEffective, ReviewMode, ReviewScope, ReviewSetting, ReviewSource};

use crate::features::session::SessionView;
use crate::features::session::state::desktop::review::Confirmation;
use crate::shared::ui;

/// What a row of the chip's card asks for: turn reviews on (`true`) or off
/// for this scope.
type Pick = Rc<dyn Fn(bool, ReviewScope, &mut Window, &mut App)>;
/// The human's answer to the confirmation: approve (`true`) or decline.
type Answer = Rc<dyn Fn(bool, &mut Window, &mut App)>;
/// The card opened: read the switch again.
type Opened = Rc<dyn Fn(&mut Window, &mut App)>;

impl SessionView {
    /// The chip, once the switch was read; nothing on a Shell without it.
    pub(super) fn render_review_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mode = *self.desktop.review().mode()?;
        let changing = self.desktop.review().changing();
        let opened = cx.entity().downgrade();
        let picked = cx.entity().downgrade();
        Some(
            review_chip(
                &mode,
                changing,
                Rc::new(move |_, cx| {
                    let _ = opened.update(cx, |this, cx| this.refresh_review_mode(cx));
                }),
                Rc::new(move |enabled, scope, _, cx| {
                    let _ =
                        picked.update(cx, |this, cx| this.request_review_mode(enabled, scope, cx));
                }),
            )
            .into_any_element(),
        )
    }

    /// The confirmation Gentle Shell asked for, over the composer.
    pub(super) fn render_review_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let confirmation = self.desktop.review().confirmation()?;
        let weak = cx.entity().downgrade();
        Some(review_confirm_card(
            confirmation,
            Rc::new(move |approved, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if approved {
                        this.confirm_review_mode(cx);
                    } else {
                        this.dismiss_review_mode(cx);
                    }
                });
            }),
        ))
    }
}

/// The chip and its card: what decides the switch now, and one row per
/// setting with the change it offers. Opening it reads the switch again,
/// since a change made in the TUI emits no event.
pub(super) fn review_chip(
    mode: &ReviewMode,
    changing: bool,
    on_open: Opened,
    on_pick: Pick,
) -> Popover {
    let (icon, color, label) = chip_look(mode.effective);
    // Its icon alone, as the permission chip: an eye when on, a struck eye
    // in amber when off; the state is in the tooltip.
    let quiet = mode.effective == ReviewEffective::Enabled;
    let trigger = Button::new("review-mode-chip")
        .debug_selector(|| "review-mode-chip".into())
        .ghost()
        .xsmall()
        .text_color(if quiet { ui::faint() } else { color })
        .icon(Icon::new(icon))
        .tooltip(t("{label} (receipt-driven development)").replace("{label}", label));
    let mode = *mode;
    let known = mode.effective != ReviewEffective::Unknown;
    Popover::new("review-mode-popover")
        .anchor(Anchor::BottomRight)
        .trigger(trigger)
        .w(rems(340. / 16.))
        .appearance(false)
        .on_open_change(move |open, window, cx| {
            if *open {
                on_open(window, cx);
            }
        })
        .content(move |_, _, cx| {
            let rows = [
                (
                    ReviewScope::Clone,
                    t("This repository"),
                    setting_label(mode.scopes.clone),
                    clone_target(&mode),
                ),
                (
                    ReviewScope::Global,
                    t("All repositories"),
                    setting_label(mode.scopes.global),
                    global_target(&mode),
                ),
            ]
            .into_iter()
            .map(|(scope, title, setting, target)| {
                let on_pick = on_pick.clone();
                let id = scope.as_str();
                h_flex()
                    .w_full()
                    .gap_3()
                    .px_2()
                    .py_2()
                    .rounded(rems(ui::ISLAND_ROW / 16.))
                    // The row under the pointer is marked, as other menu
                    // rows; only its button changes anything, so no pointer
                    // cursor and no pressed look.
                    .hover(|s| s.bg(ui::surface_hover()))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(rems(13. / 16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ui::fg())
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(rems(12. / 16.))
                                    .text_color(ui::muted())
                                    .child(setting),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("review-{id}")))
                            .ghost()
                            .xsmall()
                            .label(if target { t("Turn on") } else { t("Turn off") })
                            .disabled(changing || !known)
                            .debug_selector(move || format!("review-{id}"))
                            .on_click(cx.listener(
                                move |state: &mut PopoverState, _, window, cx| {
                                    if !target {
                                        state.dismiss(window, cx);
                                    }
                                    on_pick(target, scope, window, cx);
                                },
                            )),
                    )
            });
            let content = v_flex()
                .w_full()
                .p(rems(6. / 16.))
                .font_family(cx.theme().font_family.clone())
                .child(
                    div()
                        .px_2()
                        .pt_1p5()
                        .pb_1()
                        .child(ui::section_label(t("Review mode"))),
                )
                .child(
                    div()
                        .px_2()
                        .pb_1()
                        .text_size(rems(12. / 16.))
                        .text_color(ui::muted())
                        .whitespace_normal()
                        .child(summary(&mode)),
                )
                .children(rows)
                .child(
                    div()
                        .px_2()
                        .pt_1p5()
                        .pb_1()
                        .text_size(rems(11. / 16.))
                        .text_color(ui::faint())
                        .child(if changing {
                            t("Waiting for Gentle Shell…")
                        } else {
                            t("Gentle Shell asks you to confirm before anything changes.")
                        }),
                );
            ui::island(cx)
                .w_full()
                .child(ui::popover_in(
                    ElementId::Name("review-mode-popover-in".into()),
                    content,
                ))
                .into_any_element()
        })
}

/// The confirmation card, the server's title and message exactly as sent:
/// approving resends its token, declining sends nothing.
pub(super) fn review_confirm_card(confirmation: &Confirmation, on_answer: Answer) -> AnyElement {
    let approve = on_answer.clone();
    let decline = on_answer;
    let action = if confirmation.enabled {
        t("Turn on")
    } else {
        t("Turn off")
    };
    let card = v_flex()
        .w_full()
        .gap_3()
        .p_4()
        .rounded_2xl()
        .border_1()
        .border_color(ui::accent().opacity(0.45))
        .bg(ui::surface())
        .shadow_lg()
        .child(
            h_flex()
                .gap_2p5()
                .child(
                    div()
                        .size(rems(28. / 16.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_lg()
                        .bg(ui::accent().opacity(0.12))
                        .child(
                            Icon::new(IconName::ShieldCheck)
                                .small()
                                .text_color(ui::accent()),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(rems(14. / 16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ui::fg())
                        .child(confirmation.title.clone()),
                ),
        )
        .when(!confirmation.message.is_empty(), |this| {
            this.child(
                div()
                    .text_size(rems(13. / 16.))
                    .text_color(ui::muted())
                    .whitespace_normal()
                    .child(confirmation.message.clone()),
            )
        })
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("review-confirm")
                        .primary()
                        .small()
                        .icon(Icon::new(IconName::Check))
                        .label(action)
                        .debug_selector(|| "review-confirm".into())
                        .on_click(move |_, window, cx| approve(true, window, cx)),
                )
                .child(
                    Button::new("review-cancel")
                        .ghost()
                        .small()
                        .label(t("Cancel"))
                        .debug_selector(|| "review-cancel".into())
                        .on_click(move |_, window, cx| decline(false, window, cx)),
                ),
        );
    ui::pop_in(ElementId::Name("review-confirm-card".into()), card).into_any_element()
}

/// The chip's icon, color and text for the effective switch.
fn chip_look(effective: ReviewEffective) -> (IconName, Hsla, &'static str) {
    match effective {
        ReviewEffective::Enabled => (IconName::Eye, ui::muted(), t("Review on")),
        ReviewEffective::Disabled => (IconName::EyeOff, ui::warning(), t("Review off")),
        ReviewEffective::Unknown => (IconName::Eye, ui::faint(), t("Review ?")),
    }
}

/// What decides the switch now, in words.
fn summary(mode: &ReviewMode) -> &'static str {
    match (mode.effective, mode.source) {
        (ReviewEffective::Unknown, _) | (_, ReviewSource::Unknown) => {
            t("Gentle Shell could not read the review mode; nothing is assumed.")
        }
        (ReviewEffective::Enabled, ReviewSource::Default) => t("On: nothing turned it off."),
        (ReviewEffective::Enabled, ReviewSource::Clone) => t("On: turned on for this repository."),
        (ReviewEffective::Enabled, ReviewSource::Global) => t("On: turned on machine-wide."),
        (ReviewEffective::Disabled, ReviewSource::Global) => {
            t("Off machine-wide, which wins over this repository's setting.")
        }
        (ReviewEffective::Disabled, ReviewSource::Clone) => t("Off for this repository."),
        (ReviewEffective::Disabled, ReviewSource::Default) => t("Off."),
    }
}

fn setting_label(setting: ReviewSetting) -> &'static str {
    match setting {
        ReviewSetting::Enabled => t("On"),
        ReviewSetting::Disabled => t("Off"),
        ReviewSetting::Unset => t("Not set"),
        ReviewSetting::Unknown => t("Unknown"),
    }
}

/// This repository's row flips what applies here: reviews on turns them off,
/// and the other way round (a machine-wide off still wins, which Gentle
/// Shell's confirmation says before anything is written).
fn clone_target(mode: &ReviewMode) -> bool {
    mode.effective != ReviewEffective::Enabled
}

/// The machine-wide row flips the machine-wide setting: only an explicit off
/// turns back on.
fn global_target(mode: &ReviewMode) -> bool {
    mode.scopes.global == ReviewSetting::Disabled
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui_kit::{
        Context, Entity, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div, px,
    };
    use pi_rpc::desktop_rpc::{
        Capabilities, DesktopSession, PROMPT_COMMAND, ReviewEffective, ReviewMode, ReviewScope,
        ReviewScopes, ReviewSetting, ReviewSource, SCHEMA, WIDGET_KEY,
    };
    use pi_rpc::{UiMethod, UiRequest};
    use serde_json::{Value, json};

    use super::{clone_target, global_target, review_chip, review_confirm_card, summary};
    use crate::features::session::state::desktop::review::Review;

    fn mode(effective: ReviewEffective, source: ReviewSource, global: ReviewSetting) -> ReviewMode {
        ReviewMode {
            effective,
            source,
            scopes: ReviewScopes {
                global,
                clone: ReviewSetting::Unset,
            },
        }
    }

    #[test]
    fn each_row_offers_the_flip_of_what_it_controls() {
        let on = mode(
            ReviewEffective::Enabled,
            ReviewSource::Default,
            ReviewSetting::Unset,
        );
        assert!(!clone_target(&on) && !global_target(&on));
        let off_globally = mode(
            ReviewEffective::Disabled,
            ReviewSource::Global,
            ReviewSetting::Disabled,
        );
        assert!(clone_target(&off_globally) && global_target(&off_globally));
        assert!(summary(&off_globally).contains("machine-wide"));
        let unknown = mode(
            ReviewEffective::Unknown,
            ReviewSource::Unknown,
            ReviewSetting::Unknown,
        );
        assert!(summary(&unknown).contains("could not read"));
    }

    /// Stands in for the tab: the real slice and session, the real chip and
    /// card, and the requests the slice hands out.
    struct Host {
        review: Review,
        session: DesktopSession,
        sent: Rc<RefCell<Vec<Value>>>,
    }

    impl Host {
        fn pump(&mut self) {
            for send in self.review.pump(&mut self.session).sends {
                let payload: Value = serde_json::from_str(send.request.payload()).unwrap();
                self.sent.borrow_mut().push(payload);
            }
        }

        fn answer(&mut self, data: Value) {
            let id = self.session.pending().unwrap().0.to_string();
            let record = json!({
                "schema": SCHEMA, "kind": "response", "requestId": id, "ok": true, "data": data,
            });
            let _ = self.review.apply(
                &mut self.session,
                &UiRequest {
                    id: "ui-1".into(),
                    timeout: None,
                    method: UiMethod::SetWidget {
                        key: WIDGET_KEY.into(),
                        lines: Some(vec![record.to_string()]),
                        placement: None,
                    },
                },
            );
        }
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let opened = cx.entity().downgrade();
            let picked = cx.entity().downgrade();
            let answered = cx.entity().downgrade();
            let chip = self.review.mode().map(|mode| {
                review_chip(
                    mode,
                    self.review.changing(),
                    Rc::new(move |_, cx| {
                        let _ = opened.update(cx, |host: &mut Host, cx| {
                            host.review.refresh();
                            cx.notify();
                        });
                    }),
                    Rc::new(move |enabled, scope, _, cx| {
                        let _ = picked.update(cx, |host: &mut Host, cx| {
                            host.review.request(enabled, scope);
                            host.pump();
                            cx.notify();
                        });
                    }),
                )
            });
            let card = self.review.confirmation().map(|confirmation| {
                review_confirm_card(
                    confirmation,
                    Rc::new(move |approved, _, cx| {
                        let _ = answered.update(cx, |host: &mut Host, cx| {
                            if approved {
                                host.review.confirm();
                                host.pump();
                            } else {
                                host.review.dismiss();
                            }
                            cx.notify();
                        });
                    }),
                )
            });
            div()
                .size_full()
                .p_4()
                .flex()
                .flex_col()
                .justify_end()
                .children(card)
                .child(div().w(px(600.)).flex().justify_end().children(chip))
        }
    }

    fn open(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let sent = Rc::new(RefCell::new(Vec::new()));
        let (host, cx) = cx.add_window_view(|_, _| {
            let mut review = Review::default();
            review.set_capabilities(Capabilities {
                methods: vec!["get_review_mode".into(), "set_review_mode".into()],
                events: vec!["review_mode_changed".into()],
                schema: Some(SCHEMA.into()),
                command: Some(PROMPT_COMMAND.into()),
                widget: Some(WIDGET_KEY.into()),
            });
            Host {
                review,
                session: DesktopSession::new(),
                sent,
            }
        });
        host.update(cx, |host, cx| {
            host.pump();
            host.answer(json!({
                "effective": "enabled", "source": "default",
                "scopes": {"global": "unset", "clone": "unset"},
            }));
            cx.notify();
        });
        cx.run_until_parked();
        (host, cx)
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        cx.run_until_parked();
        let at = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("nothing drawn as {selector}"))
            .center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
    }

    fn sent(host: &Entity<Host>, cx: &mut VisualTestContext) -> Vec<Value> {
        host.read_with(cx, |host, _| host.sent.borrow().clone())
    }

    fn confirmation(token: &str) -> Value {
        json!({
            "status": "confirmation_required", "enabled": false, "scope": "clone",
            "mode": {"effective": "enabled", "source": "default",
                     "scopes": {"global": "unset", "clone": "unset"}},
            "confirmation": {
                "title": "Turn review mode off for this repository?",
                "message": "Reviews stop here; other repositories keep theirs.",
                "token": token,
            },
        })
    }

    #[gpui::test]
    fn turning_reviews_off_asks_first_and_resends_only_the_approved_token(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        click(cx, "review-mode-chip");
        // Opening read the switch again.
        host.update(cx, |host, _| host.pump());
        assert_eq!(sent(&host, cx).last().unwrap()["method"], "get_review_mode");
        host.update(cx, |host, cx| {
            host.answer(json!({
                "effective": "enabled", "source": "default",
                "scopes": {"global": "unset", "clone": "unset"},
            }));
            cx.notify();
        });

        click(cx, "review-clone");
        assert!(cx.debug_bounds("review-clone").is_none());
        assert!(cx.debug_bounds("review-global").is_none());
        let asked = sent(&host, cx).last().unwrap().clone();
        assert_eq!(asked["method"], "set_review_mode");
        assert_eq!(asked["params"], json!({"enabled": false, "scope": "clone"}));

        // Gentle Shell's dialog, shown as sent; Cancel resends nothing.
        host.update(cx, |host, cx| {
            host.answer(confirmation("t1"));
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("review-confirm").is_some());
        assert!(cx.debug_bounds("review-clone").is_none());
        let before = sent(&host, cx).len();
        click(cx, "review-cancel");
        assert_eq!(sent(&host, cx).len(), before);
        assert!(cx.debug_bounds("review-confirm").is_none());

        // Asked again; this time approved, then a stale token asks once more.
        host.update(cx, |host, cx| {
            host.review.request(false, ReviewScope::Clone);
            host.pump();
            host.answer(confirmation("t2"));
            cx.notify();
        });
        click(cx, "review-confirm");
        let resent = sent(&host, cx).last().unwrap().clone();
        assert_eq!(resent["params"]["confirmationToken"], "t2");
        host.update(cx, |host, cx| {
            host.answer(confirmation("t3"));
            cx.notify();
        });
        click(cx, "review-confirm");
        assert_eq!(
            sent(&host, cx).last().unwrap()["params"]["confirmationToken"],
            "t3"
        );
        host.update(cx, |host, cx| {
            host.answer(json!({
                "status": "applied", "enabled": false, "scope": "clone", "changed": true,
                "mode": {"effective": "disabled", "source": "clone",
                         "scopes": {"global": "unset", "clone": "disabled"}},
            }));
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("review-confirm").is_none());
        host.read_with(cx, |host, _| {
            assert_eq!(
                host.review.mode().unwrap().effective,
                ReviewEffective::Disabled
            );
        });

        // The global off action also closes the card before confirmation.
        click(cx, "review-mode-chip");
        host.update(cx, |host, cx| {
            host.pump();
            host.answer(json!({
                "effective": "disabled", "source": "clone",
                "scopes": {"global": "unset", "clone": "disabled"},
            }));
            cx.notify();
        });
        click(cx, "review-global");
        assert!(cx.debug_bounds("review-global").is_none());
        assert_eq!(
            sent(&host, cx).last().unwrap()["params"],
            json!({"enabled": false, "scope": "global"})
        );
        host.update(cx, |host, cx| {
            let mut response = confirmation("global");
            response["scope"] = json!("global");
            host.answer(response);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("review-confirm").is_some());
        assert!(cx.debug_bounds("review-global").is_none());
        let before = sent(&host, cx).len();
        click(cx, "review-cancel");
        assert_eq!(sent(&host, cx).len(), before);

        // Turning on retains the existing open-popover behavior.
        click(cx, "review-mode-chip");
        host.update(cx, |host, cx| {
            host.pump();
            host.answer(json!({
                "effective": "disabled", "source": "clone",
                "scopes": {"global": "unset", "clone": "disabled"},
            }));
            cx.notify();
        });
        click(cx, "review-clone");
        assert_eq!(
            sent(&host, cx).last().unwrap()["params"],
            json!({"enabled": true, "scope": "clone"})
        );
        assert!(cx.debug_bounds("review-clone").is_some());
    }
}
