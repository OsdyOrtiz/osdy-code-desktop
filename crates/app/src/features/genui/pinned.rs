//! The conversation's side panel for pinned surfaces: the same live views
//! as in the conversation (an agent's update shows here at once), docked
//! beside it so they stay in sight while the conversation scrolls. Several
//! pinned surfaces are tabs; a tab's × unpins it.

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Surface;
use super::diagram::DiagramEvent;
use crate::shared::ui;

/// `pinned` as (id, surface, title), `shown` the tab on top; a surface
/// shown expanded gets a note in its place (a view is drawn in one place).
pub fn panel(
    pinned: &[(String, Surface, String)],
    shown: &str,
    expanded: Option<&str>,
) -> AnyElement {
    let tabs = h_flex()
        .id("genui-pinned-tabs")
        .px_2()
        .py_1p5()
        .gap_1()
        .min_w_0()
        .overflow_x_scroll()
        .child(
            Icon::new(IconName::Pin)
                .xsmall()
                .flex_shrink_0()
                .text_color(ui::muted()),
        )
        .children(pinned.iter().enumerate().map(|(i, (id, surface, title))| {
            let active = id == shown;
            let (show, unpin) = (surface.clone(), surface.clone());
            let tab_selector = format!("genui-pinned-tab-{id}");
            let close_selector = format!("genui-unpin-{id}");
            h_flex()
                .id(("genui-pinned-tab", i))
                .debug_selector(move || tab_selector)
                .flex_shrink_0()
                .max_w(rems(180. / 16.))
                .gap_1()
                .pl_2()
                .pr_1()
                .py_0p5()
                .rounded_md()
                .cursor_pointer()
                .text_size(rems(12. / 16.))
                .text_color(if active { ui::fg() } else { ui::muted() })
                .when(active, |d| d.bg(ui::raised()))
                .hover(|d| d.bg(ui::surface_hover()))
                .child(div().min_w_0().truncate().child(title.clone()))
                .child(
                    div()
                        .id(("genui-unpin", i))
                        .debug_selector(move || close_selector)
                        .flex_shrink_0()
                        .rounded_sm()
                        .text_color(ui::faint())
                        .hover(|d| d.text_color(ui::fg()))
                        .child(Icon::new(IconName::X).xsmall())
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(crate::shared::i18n::t(
                                "Unpin",
                            ))
                            .build(window, cx)
                        })
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            unpin.emit(DiagramEvent::Pin(false), cx);
                        }),
                )
                // Pinning again brings it to the front.
                .on_click(move |_, _, cx| show.emit(DiagramEvent::Pin(true), cx))
        }));
    let body = pinned
        .iter()
        .find(|(id, ..)| id == shown)
        .map(|(id, surface, _)| {
            if expanded == Some(id.as_str()) {
                div()
                    .p_4()
                    .text_size(rems(12. / 16.))
                    .text_color(ui::faint())
                    .child(crate::shared::i18n::t("Open in the expanded view"))
                    .into_any_element()
            } else {
                surface
                    .view()
                    .cached(StyleRefinement::default().size_full())
                    .into_any_element()
            }
        });
    v_flex()
        .id("genui-pinned")
        .debug_selector(|| "genui-pinned".into())
        .w(relative(0.4))
        .min_w(rems(300. / 16.))
        .max_w(rems(560. / 16.))
        .flex_shrink_0()
        .h_full()
        .border_l_1()
        .border_color(ui::border())
        .child(tabs)
        .child(div().flex_1().min_h_0().px_2().pb_2().children(body))
        .into_any_element()
}
