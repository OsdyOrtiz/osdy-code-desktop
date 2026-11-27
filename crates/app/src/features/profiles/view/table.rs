//! The selected profile: its scope card, the notes the screen must show and
//! the route table.

use gentle_core::profiles::{
    ORCHESTRATOR, OrchestratorMode, PinRead, RouteEntry, evaluate_pin, is_review_role,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::profiles::profile_msg;
use crate::features::profiles::{ALL_ROW, ProfilesView};
use crate::shared::{
    i18n::{language, t},
    ui,
};

use super::{EFFORT_W, TIER_W, route_cells, route_columns, route_name_column};

impl ProfilesView {
    fn render_row(&self, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let (label, hint) = match key {
            ORCHESTRATOR => (t("orchestrator").to_string(), Some("settings.json")),
            ALL_ROW => (t("every agent").to_string(), Some(t("sets all rows"))),
            k if is_review_role(k) => (k.to_string(), Some(t("review role"))),
            k => (k.to_string(), None),
        };
        let custom = self.custom_target.as_deref() == Some(key);
        let row = v_flex()
            .rounded_lg()
            .hover(|s| s.bg(ui::surface_hover()))
            .when(custom, |d| d.bg(ui::raised()))
            .child(
                route_columns()
                    .px_2()
                    .py_1p5()
                    .child(
                        // The hint sits under the name so the column stays
                        // narrow enough for the name itself.
                        route_name_column()
                            .child(
                                div()
                                    .truncate()
                                    .font_family(ui::mono())
                                    .text_size(ui::code_rems(12.))
                                    .text_color(if key == ALL_ROW {
                                        ui::muted()
                                    } else {
                                        ui::fg()
                                    })
                                    .child(label),
                            )
                            .when_some(hint, |d, hint| {
                                d.child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .text_color(ui::muted())
                                        .child(hint),
                                )
                            }),
                    )
                    .child(
                        route_cells()
                            .child(self.render_model_cell(key, cx))
                            .child(self.render_effort_cell(key, cx))
                            .child(self.render_tier_cell(key, cx)),
                    ),
            )
            .when(custom, |d| {
                d.child(
                    h_flex()
                        .px_2()
                        .pb_2()
                        .gap_2()
                        .child(
                            ui::field()
                                .flex_1()
                                .min_w_0()
                                .child(Input::new(&self.custom_input).appearance(false).small()),
                        )
                        .child(
                            Button::new("custom-apply")
                                .small()
                                .outline()
                                .icon(Icon::new(IconName::Check))
                                .label(t("Set"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.submit_custom(window, cx)
                                })),
                        )
                        .child(
                            Button::new("custom-cancel")
                                .small()
                                .ghost()
                                .label(t("Cancel"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.custom_target = None;
                                    cx.notify();
                                })),
                        ),
                )
            });
        ui::appear(SharedString::from(format!("route-{key}")), row).into_any_element()
    }

    /// One choice of a workflow setting: outlined when chosen, in `warning`
    /// when the chosen one is not saved yet.
    fn workflow_option(
        &self,
        id: String,
        label: &'static str,
        tooltip: &'static str,
        on: bool,
        edited: bool,
    ) -> Button {
        Button::new(SharedString::from(id))
            .xsmall()
            .when(on, |b| b.outline().selected(true))
            .when(!on, |b| b.ghost())
            .when(on && edited, |b| b.text_color(ui::warning()))
            .label(t(label))
            .tooltip(t(tooltip))
            .disabled(self.busy)
    }

    /// The shown profile's orchestrator mode (gentle-pi's per-profile
    /// `workflow`), edited in the draft like a route.
    fn render_mode(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let edited = self.draft_mode != self.stored_mode;
        let option = |mode: OrchestratorMode, label: &'static str, tooltip: &'static str| {
            self.workflow_option(
                format!("mode-{}", mode.as_str()),
                label,
                tooltip,
                self.draft_mode == mode,
                edited,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.draft_mode = mode;
                cx.notify();
            }))
        };
        let organic = option(
            OrchestratorMode::Organic,
            "Organic",
            "The orchestrator may implement small changes itself",
        );
        let delegation = option(
            OrchestratorMode::FullDelegation,
            "Full delegation",
            "The orchestrator never implements: every change goes to a bounded writer",
        );
        h_flex()
            .gap_2()
            .text_xs()
            .text_color(ui::muted())
            .child(t("Orchestrator"))
            .child(organic)
            .child(delegation)
    }

    /// The shown profile's Controlled Engineering option (stored beside the
    /// mode in its `workflow` entry), edited in the draft like the mode.
    fn render_controlled(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let edited = self.draft_controlled != self.stored_controlled;
        let option = |enabled: bool, label: &'static str, tooltip: &'static str| {
            self.workflow_option(
                format!("controlled-{label}"),
                label,
                tooltip,
                self.draft_controlled == enabled,
                edited,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.draft_controlled = enabled;
                cx.notify();
            }))
        };
        let off = option(false, "Off", "No extra authoring rules");
        let on = option(
            true,
            "On",
            "Adds rules against unrequested machinery: to the orchestrator in Organic mode, and to writer and verify agents",
        );
        h_flex()
            .gap_2()
            .text_xs()
            .text_color(ui::muted())
            .child(t("Controlled Engineering"))
            .child(off)
            .child(on)
    }

    fn render_scope(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self.selected.clone().unwrap_or_default();
        let mut parts: Vec<(&'static str, Hsla)> = Vec::new();
        if self.in_session.as_deref() == Some(name.as_str()) {
            parts.push(("Used in this session", ui::success()));
        }
        if self.file.active.as_deref() == Some(name.as_str()) {
            parts.push(("Default for new sessions", ui::accent()));
        }
        if self.pin_winner().as_deref() == Some(name.as_str()) {
            parts.push(("Used in this repo", ui::warning()));
        }
        let default_line = match &self.file.active {
            Some(active) => t("Global default: {active}").replace("{active}", active),
            None => t("No global default profile").into(),
        };
        let pin_line = match &self.pin {
            None => t("Repo pin: none (not inside a Git worktree)").to_string(),
            Some(status) => {
                let layer = |read: &PinRead| match read {
                    PinRead::Missing => t("none").to_string(),
                    PinRead::Invalid => t("invalid file").to_string(),
                    PinRead::Valid(p) => p.clone(),
                };
                profile_msg!(
                    "Repo pin: local {local} · declaration {repo}",
                    local = layer(&status.local),
                    repo = layer(&status.repo)
                )
                .render()
            }
        };
        ui::card()
            .p_3()
            .gap_1()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ui::fg())
                            .child(name),
                    )
                    .when(parts.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(ui::muted())
                                .child(t("Not in use. Pick a button below to use it.")),
                        )
                    })
                    .children(
                        parts
                            .into_iter()
                            .map(|(part, color)| ui::chip(t(part), color)),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_x_3()
                    .gap_y_0p5()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(default_line)
                    .child(pin_line),
            )
            .child(self.render_mode(cx))
            .child(self.render_controlled(cx))
    }

    pub(super) fn render_table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = language();
        let notes: Vec<String> = self
            .notices
            .iter()
            .map(|note| note.render())
            .chain(evaluate_pin(self.pin.as_ref(), &self.file).notes_in(language))
            .chain(self.draft.unmatched_notes_in(&self.names, language))
            .collect();
        let orchestrator_now = self.pi_default.clone();
        v_flex()
            .gap_3()
            .child(self.render_scope(cx))
            .children(notes.into_iter().map(|note| {
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(ui::warning().opacity(0.22))
                    .bg(ui::warning().opacity(0.08))
                    .text_xs()
                    .text_color(ui::warning())
                    .child(Icon::new(IconName::TriangleAlert).xsmall().flex_shrink_0())
                    .child(div().flex_1().min_w_0().child(note))
            }))
            .child(
                h_flex()
                    .px_1()
                    .gap_2()
                    .child(ui::section_label(t("Routes")))
                    .child(div().flex_1())
                    .child(
                        Button::new("all-fast")
                            .small()
                            .ghost()
                            .icon(Icon::new(IconName::Zap))
                            .label(t("All Fast"))
                            .tooltip(t("Set every compatible OpenAI route to Fast (draft only)"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.all_fast(cx))),
                    ),
            )
            .child(
                ui::card()
                    .p_1()
                    .gap_0p5()
                    .child(
                        route_columns()
                            .px_2()
                            .pt_1p5()
                            .pb_1()
                            .child(route_name_column().child(ui::section_label(t("Route"))))
                            .child(
                                route_cells()
                                    .child(div().flex_1().min_w_0().px_1().child(ui::section_label(t("Model"))))
                                    .child(
                                        div()
                                            .w(rems(EFFORT_W / 16.))
                                            .flex_shrink_0()
                                            .px_1()
                                            .child(ui::section_label(t("Effort"))),
                                    )
                                    .child(
                                        div()
                                            .w(rems(TIER_W / 16.))
                                            .flex_shrink_0()
                                            .px_1()
                                            .child(ui::section_label(t("Tier"))),
                                    ),
                            ),
                    )
                    .child(self.render_row(ORCHESTRATOR, cx))
                    .child(self.render_row(ALL_ROW, cx))
                    .when(!self.names.is_empty(), |d| {
                        d.child(div().mx_2().my_1().h(px(1.)).bg(ui::border()))
                            .child(
                                h_flex()
                                    .px_2()
                                    .pt_0p5()
                                    .gap_2()
                                    .child(ui::section_label(t("Agents")))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ui::faint())
                                            .child(self.names.len().to_string()),
                                    ),
                            )
                    })
                    .children(self.names.iter().map(|key| self.render_row(key, cx))),
            )
            .child(
                div()
                    .px_1()
                    .text_xs()
                    .text_color(ui::faint())
                    .child(t("Pi default orchestrator now: {model}. Fast applies only to openai / openai-codex GPT-5.6 / 6 / 6.1 models; the selector is a request, not a billing guarantee.")
                        .replace("{model}", &orchestrator_now.map_or_else(|| t("not set").to_string(), |e| RouteEntry::format_selection_in(Some(&e), language)))),
            )
    }
}
