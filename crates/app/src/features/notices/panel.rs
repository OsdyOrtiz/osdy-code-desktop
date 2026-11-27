//! The notification list inside the bell's popover: header actions (mark all
//! read, clear) and one row per notice, which jumps to its session, or says
//! why it can't (its pane closed, or shows another session now).

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::popover::PopoverState;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shared::{
    i18n::{language, t},
    ui,
};
use gentle_core::transcript::NoticeLevel;

use super::{Notices, OnSelect, Reach, ReachOf};

/// "now", "42s", "5m", "3h", "2d".
fn ago(elapsed: Duration, language: gentle_core::i18n::Language) -> String {
    let s = elapsed.as_secs();
    match s {
        0..10 => gentle_core::i18n::tr(language, "now").into(),
        10..60 => format!("{s}s"),
        60..3600 => format!("{}m", s / 60),
        3600..86400 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

fn level_style(level: NoticeLevel) -> (IconName, Hsla) {
    match level {
        NoticeLevel::Info => (IconName::Info, ui::info()),
        NoticeLevel::Warning => (IconName::TriangleAlert, ui::warning()),
        NoticeLevel::Error => (IconName::CircleX, ui::danger()),
    }
}

/// What a row says, and a click explains, when its session can't be shown.
fn gone_texts(reach: Reach) -> Option<(&'static str, &'static str)> {
    match reach {
        Reach::Open => None,
        Reach::SessionGone => Some((
            t("session no longer open"),
            t("The session that sent this is no longer open: its pane shows another one now."),
        )),
        Reach::PaneClosed => Some((
            t("pane closed"),
            t("The pane that sent this notification was closed."),
        )),
    }
}

pub(super) fn panel(
    store: &Entity<Notices>,
    on_select: OnSelect,
    reach: ReachOf,
    cx: &mut Context<PopoverState>,
) -> Div {
    let notices = store.read(cx);
    let unread = notices.unread();
    let now = Instant::now();
    let action = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .px_2()
            .h(rems(24. / 16.))
            .flex()
            .items_center()
            .rounded(rems(6. / 16.))
            .text_size(rems(11.5 / 16.))
            .text_color(ui::muted())
            .cursor_pointer()
            .hover(|s| s.bg(ui::surface_hover()).text_color(ui::fg()))
            .child(label)
    };
    let header = h_flex()
        .px_3()
        .pt_2p5()
        .pb_2()
        .gap_2()
        .child(
            div()
                .text_size(rems(13. / 16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::fg())
                .child(t("Notifications")),
        )
        .when(unread > 0, |d| {
            d.child(ui::chip(
                t("{count} new").replace("{count}", &unread.to_string()),
                ui::petal(),
            ))
        })
        .child(div().flex_1())
        .when(unread > 0, |d| {
            let store = store.clone();
            d.child(
                action("notices-read", t("Mark all read")).on_click(cx.listener(
                    move |_, _, _, cx| {
                        store.update(cx, |s, cx| {
                            s.mark_all_read();
                            cx.notify();
                        });
                        cx.notify();
                    },
                )),
            )
        })
        .when(!notices.is_empty(), |d| {
            let store = store.clone();
            d.child(
                // Nothing is left to read: close the list instead of leaving
                // an empty card open.
                action("notices-clear", t("Clear")).on_click(cx.listener(
                    move |state: &mut PopoverState, _, window, cx| {
                        store.update(cx, |s, cx| {
                            s.clear();
                            cx.notify();
                        });
                        state.dismiss(window, cx);
                    },
                )),
            )
        });

    let body = if notices.is_empty() {
        v_flex()
            .py_8()
            .gap_2()
            .items_center()
            .text_color(ui::faint())
            .child(Icon::new(IconName::BellOff).small())
            .child(
                div()
                    .text_size(rems(12. / 16.))
                    .child(t("No notifications")),
            )
            .into_any_element()
    } else {
        let rows = notices.iter().map(|notice| {
            let (icon, color) = level_style(notice.level);
            let id = notice.id;
            let gone = notice
                .origin
                .as_ref()
                .and_then(|origin| gone_texts(reach(origin, cx)));
            let store = store.clone();
            let on_select = on_select.clone();
            let reach = reach.clone();
            let mut meta = notice.source.to_string();
            if let Some((marker, _)) = gone {
                if !meta.is_empty() {
                    meta.push_str("  ·  ");
                }
                meta.push_str(marker);
            }
            if !meta.is_empty() {
                meta.push_str("  ·  ");
            }
            meta.push_str(&ago(now.saturating_duration_since(notice.at), language()));
            if notice.repeats > 0 {
                meta.push_str(&format!("  ·  ×{}", notice.repeats + 1));
            }
            h_flex()
                .id(("notice", id as usize))
                .w_full()
                .items_start()
                .gap_2p5()
                .px_2()
                .py_2()
                .rounded(rems(8. / 16.))
                .cursor_pointer()
                .hover(|s| s.bg(ui::surface_hover()))
                .on_click(cx.listener(move |state: &mut PopoverState, _, window, cx| {
                    store.update(cx, |s, cx| {
                        s.mark_read(id);
                        cx.notify();
                    });
                    // Asked again: the pane may have moved on since the list drew.
                    let target = store
                        .read(cx)
                        .iter()
                        .find(|n| n.id == id)
                        .and_then(|n| n.origin.as_ref())
                        .map(|origin| (origin.pane, reach(origin, cx)));
                    if let Some((pane, reach)) = target {
                        state.dismiss(window, cx);
                        match gone_texts(reach) {
                            None => on_select(pane, window, cx),
                            Some((_, why)) => window.push_notification(Notification::info(why), cx),
                        }
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .mt(px(1.))
                        .size(rems(22. / 16.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(rems(6. / 16.))
                        .bg(color.opacity(0.12))
                        .child(Icon::new(icon).xsmall().text_color(color)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(rems(12.5 / 16.))
                                .line_height(relative(1.4))
                                .text_color(if notice.read { ui::muted() } else { ui::fg() })
                                .child(notice.message.clone()),
                        )
                        .child(
                            div()
                                .text_size(rems(11. / 16.))
                                .text_color(ui::faint())
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(meta),
                        ),
                )
                .child(
                    div()
                        .mt(rems(7. / 16.))
                        .size(rems(6. / 16.))
                        .flex_none()
                        .rounded_full()
                        .when(!notice.read, |d| d.bg(ui::petal())),
                )
        });
        v_flex()
            .id("notices-list")
            .max_h(rems(420. / 16.))
            .overflow_y_scroll()
            .px_1()
            .pb_1()
            .children(rows)
            .into_any_element()
    };

    v_flex()
        .w_full()
        .font_family(cx.theme().font_family.clone())
        .child(header)
        .child(div().h(px(1.)).bg(ui::border()))
        .child(div().pt_1().child(body))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ago;

    #[test]
    fn relative_times() {
        let ago = |secs| {
            ago(
                Duration::from_secs(secs),
                gentle_core::i18n::Language::English,
            )
        };
        assert_eq!(ago(3), "now");
        assert_eq!(ago(42), "42s");
        assert_eq!(ago(300), "5m");
        assert_eq!(ago(7200), "2h");
        assert_eq!(ago(200_000), "2d");
    }
}
