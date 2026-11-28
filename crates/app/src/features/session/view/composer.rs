//! The composer: attachments, text area and its bottom bar.

use crate::shared::i18n::t;
use std::time::{Duration, Instant};

/// The focus pulse's length.
const PULSE: Duration = Duration::from_millis(460);
/// A typing particle's flight, the least time between two, and how many fly.
const PARTICLE_LIFE: Duration = Duration::from_millis(900);
const PARTICLE_GAP: Duration = Duration::from_millis(90);
const PARTICLES_MAX: usize = 10;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputToken, Textarea};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::activity::{ChipPopoverOptions, chip_popover};
use super::dialogs::checkbox_box;
use super::{FileStrip, NARROW, TINY, file_tile, image_tile, number_badge};
use crate::features::session::SessionView;
use crate::features::session::attachments::{self, PastedText};
use crate::features::session::submit::SKILL_TOKEN_ID;
use crate::shared::kisses::{noise, paint_particle, paw};
use crate::shared::ui;
use gentle_core::input_style::InputStyle;
use gentle_core::transcript::format_tokens;

// The Kit uses the global muted foreground for placeholders, so draw this
// hint separately to keep other inputs (and the composer's caret) unchanged.
fn composer_hint(hint: &'static str, elapsed: Option<Duration>) -> Div {
    let mut text = hint.to_string();
    let mut highlights = Vec::new();
    if let Some(elapsed) = elapsed
        && let Some(stem) = hint.strip_suffix('…')
    {
        use gentle_core::composer_placeholder::ellipsis_frame;
        let visible_end = ellipsis_frame(hint, elapsed).len();
        text = format!("{stem}...");
        // Hidden glyphs still occupy their full width, even in a custom font.
        if visible_end < text.len() {
            highlights.push((
                visible_end..text.len(),
                HighlightStyle {
                    color: Some(ui::faint().opacity(0.)),
                    ..Default::default()
                },
            ));
        }
    }
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .px_2()
        .text_color(ui::faint())
        .overflow_hidden()
        .child(
            div()
                .truncate()
                .child(StyledText::new(text).with_highlights(highlights)),
        )
}

impl SessionView {
    fn can_animate_composer_hint(&self, cx: &App) -> bool {
        (self.web_shown || self.canvas.is_some())
            && self.extension.dialogs.is_empty()
            && !cx.reduce_motion()
            && self.composer.read(cx).value().is_empty()
    }

    pub(super) fn step_composer_hint(
        &mut self,
        veil: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Duration> {
        use gentle_core::composer_placeholder::DOT_INTERVAL;
        if veil || !window.is_window_active() || !self.can_animate_composer_hint(cx) {
            self.composer_placeholder_since = None;
            self.composer_placeholder_tick = None;
            return None;
        }

        let elapsed = self
            .composer_placeholder_since
            .get_or_insert_with(Instant::now)
            .elapsed();
        // Unrelated redraws don't postpone the tick. Only ask again if the
        // composer actually renders, and recheck before notifying.
        if self.composer_placeholder_tick.is_none() {
            let wait = DOT_INTERVAL
                - Duration::from_millis((elapsed.as_millis() % DOT_INTERVAL.as_millis()) as u64);
            self.composer_placeholder_tick = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                let _ = this.update(cx, |this, cx| {
                    this.composer_placeholder_tick = None;
                    if this.can_animate_composer_hint(cx) && this.window_active(cx) {
                        cx.notify();
                    } else {
                        this.composer_placeholder_since = None;
                    }
                });
            }));
        }
        Some(elapsed)
    }

    /// While the agent works a light runs round the composer's action
    /// button (stop, or send while you steer), just outside it, on the live
    /// overlay. Round buttons get a round lap (the orbit's corner radius
    /// clamps to a circle); Bar's framed Stop follows its corners; Prompt's
    /// words get none.
    fn action_ring(&self, style: InputStyle) -> Option<Div> {
        /// How far outside the button the light runs.
        const GAP: f32 = 3.;
        let radius = match style {
            InputStyle::Prompt => return None,
            InputStyle::Bar => rems((6. + GAP) / 16.),
            _ => rems(99.),
        };
        self.busy().then(|| {
            self.live_slots
                .slot("action-ring", move || {
                    ui::orbit(ui::petal(), radius).into_any_element()
                })
                .absolute()
                .top(px(-GAP))
                .left(px(-GAP))
                .right(px(-GAP))
                .bottom(px(-GAP))
        })
    }

    /// Prefer the session feed's window, falling back to the current model.
    pub(super) fn context_window(&self) -> Option<u64> {
        self.usage.context_window.or_else(|| {
            self.models
                .current
                .as_ref()
                .map(|model| model.context_window)
                .filter(|window| *window > 0)
        })
    }

    fn render_context_control(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let compacting = self.usage.compacting;
        let auto = self.usage.auto_compaction;
        let ready = self.backend.is_ready();
        let prefix = if self.usage.context_estimated {
            "~"
        } else {
            ""
        };
        let used = match self.usage.context_percent {
            Some(p) => format!("{prefix}{p:.0}%"),
            None => "–".into(),
        };
        let label = if compacting {
            t("compacting…").to_string()
        } else {
            used.clone()
        };
        let ring_rows = [
            (
                "used-tokens",
                t("Used tokens"),
                self.usage
                    .context_tokens
                    .map(|tokens| format!("{prefix}{}", format_tokens(tokens))),
            ),
            (
                "context-window",
                t("Context window"),
                self.context_window().map(format_tokens),
            ),
            (
                "usage",
                t("Usage"),
                self.usage
                    .context_percent
                    .map(|p| format!("{prefix}{p:.0}%")),
            ),
        ];
        let percent = self.usage.context_percent.unwrap_or(0.);
        let color = if percent >= 90. {
            ui::danger()
        } else if percent >= 70. {
            ui::warning()
        } else {
            ui::success()
        };
        let weak = cx.entity().downgrade();
        let options = ChipPopoverOptions::new("context-ring", IconName::Gauge, color, label)
            .title(t("Context"))
            .ring(
                percent as f32,
                cx.entity(),
                self.usage.context_details_open,
                self.usage.context_ring_hovered,
                compacting,
                cx,
            );
        chip_popover(options, move || {
            let compact = weak.clone();
            let toggle = weak.clone();
            let mut rows = Vec::new();
            rows.extend(ring_rows.iter().filter_map(|(id, label, value)| {
                let value = value.clone()?;
                let selector = format!("context-ring-{id}-{value}");
                Some(
                    h_flex()
                        .text_size(rems(12. / 16.))
                        .child(div().flex_1().text_color(ui::muted()).child(*label))
                        .child(
                            div()
                                .debug_selector(move || selector.clone())
                                .font_family(ui::mono())
                                .text_color(ui::fg())
                                .child(value),
                        )
                        .into_any_element(),
                )
            }));
            rows.push(
                div()
                    .debug_selector(|| "context-ring-progress".into())
                    .w_full()
                    .h(px(5.))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(ui::border_strong())
                    .when(percent > 0. && percent.is_finite(), |track| {
                        let tone = if percent >= 90. {
                            "context-progress-danger"
                        } else if percent >= 70. {
                            "context-progress-warning"
                        } else {
                            "context-progress-success"
                        };
                        track.child(
                            div()
                                .debug_selector(move || tone.into())
                                .h_full()
                                .w(relative(percent.clamp(0., 100.) as f32 / 100.))
                                .rounded_full()
                                .bg(color)
                                .child(
                                    div()
                                        .debug_selector(|| "context-progress-fill".into())
                                        .size_full(),
                                ),
                        )
                    })
                    .into_any_element(),
            );
            let advice = if percent >= 90. {
                t("Context nearly full. Compact or start a new session.")
            } else if percent >= 70. {
                t("Consider compacting or starting a new session.")
            } else {
                t("Compacting summarizes the conversation to free context.")
            };
            let instruction = t("Add what to keep with /compact <instructions>.");
            rows.push(
                v_flex()
                    .text_size(rems(12. / 16.))
                    .text_color(ui::muted())
                    .whitespace_normal()
                    .child(div().debug_selector(move || advice.into()).child(advice))
                    .child(
                        div()
                            .debug_selector(move || instruction.into())
                            .child(instruction),
                    )
                    .into_any_element(),
            );
            rows.extend(vec![
                h_flex()
                    .id("context-compact-control")
                    .debug_selector(|| "context-compact-control".into())
                    .pt_1()
                    .child(
                        Button::new("context-compact")
                            .small()
                            .primary()
                            .cursor(CursorStyle::Arrow)
                            .icon(Icon::new(IconName::ChevronsDownUp))
                            .label(if compacting {
                                t("Compacting…")
                            } else {
                                t("Compact now")
                            })
                            .loading(compacting)
                            .disabled(compacting || !ready)
                            .on_click(move |_, _, cx| {
                                let _ = compact.update(cx, |this, cx| this.compact(None, cx));
                            }),
                    )
                    .into_any_element(),
                h_flex()
                    .id("context-auto-compaction")
                    .debug_selector(|| "context-auto-compaction".into())
                    .gap_2()
                    .pt_1()
                    .cursor(CursorStyle::Arrow)
                    .text_size(rems(12. / 16.))
                    .text_color(ui::fg())
                    .when(auto.is_none() || !ready, |d| d.opacity(0.5))
                    .child(checkbox_box(auto.unwrap_or(false)))
                    .child(t("Compact automatically when nearly full"))
                    .when_some(auto.filter(|_| ready), |d, enabled| {
                        d.on_click(move |_, _, cx| {
                            let _ = toggle
                                .update(cx, |this, cx| this.set_auto_compaction(!enabled, cx));
                        })
                    })
                    .into_any_element(),
            ]);
            rows
        })
    }

    /// The strip's tiles: numbered image thumbnails, then the files in the
    /// same tile, each with its ×.
    fn render_attachments(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.attachments.is_empty() && self.files.is_empty() && self.pasted.is_empty() {
            return None;
        }
        let images: Vec<_> = self
            .attachments
            .iter()
            .enumerate()
            .map(|(ix, attachment)| {
                image_tile(
                    Some(attachment.image.clone()),
                    rems(64. / 16.),
                    rems(64. / 16.),
                )
                .debug_selector(move || format!("composer-image-{ix}"))
                .child(number_badge(ix + 1))
                .child(
                    remove_button(
                        ("attachment-remove", ix),
                        cx.listener(move |this, _, _, cx| this.remove_attachment(ix, cx)),
                    )
                    .debug_selector(move || format!("attachment-remove-{ix}")),
                )
            })
            .collect();
        let files = self.files.iter().enumerate().map(|(ix, file)| {
            let tip: SharedString = format!(
                "{} ({})",
                file.path.display(),
                attachments::human_size(file.bytes)
            )
            .into();
            file_tile(("file-tile", ix), &file.name, tip, FileStrip::Composer)
                .debug_selector(move || format!("composer-file-{ix}"))
                .child(
                    remove_button(
                        ("file-remove", ix),
                        cx.listener(move |this, _, _, cx| this.remove_file(ix, cx)),
                    )
                    .debug_selector(move || format!("file-remove-{ix}")),
                )
        });
        let pastes = self.pasted.iter().enumerate().map(|(ix, paste)| {
            pasted_card(ix + 1, paste).child(remove_button(
                ("pasted-remove", ix),
                cx.listener(move |this, _, window, cx| this.remove_pasted(ix, window, cx)),
            ))
        });
        Some(
            h_flex()
                .w_full()
                .flex_wrap()
                .gap_2()
                .children(images)
                .children(files)
                .children(pastes),
        )
    }

    /// The text field; a click anywhere in it focuses the text area.
    fn render_text_field(
        &self,
        ready: bool,
        hint_elapsed: Option<Duration>,
        cx: &mut Context<Self>,
    ) -> Div {
        let weak = cx.entity().downgrade();
        let hint = t(self.composer_placeholder);
        div()
            .relative()
            .min_w_0()
            .text_size(rems(14. / 16.))
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.composer
                        .update(cx, |state, cx| state.focus(window, cx));
                }),
            )
            .child(
                Textarea::new(&self.composer)
                    .appearance(false)
                    .disabled(!ready)
                    .aria_label(hint)
                    // A pasted text's `#N` chip, or — in the theme's own
                    // accent — the `/skill:name` the prompt invokes.
                    .token(|token, _, _| {
                        let (tint, icon) = if token.token().id().as_ref() == SKILL_TOKEN_ID {
                            (ui::accent(), IconName::Zap)
                        } else {
                            (ui::info(), IconName::FileText)
                        };
                        let chip = InputToken::new(token).icon(icon).text_color(tint);
                        if token.is_selected() {
                            chip
                        } else {
                            chip.bg(tint.opacity(0.12)).border_color(tint.opacity(0.22))
                        }
                    })
                    .on_paste(move |item, window, cx| {
                        weak.update(cx, |this, cx| this.paste_clipboard(item, window, cx))
                            .unwrap_or(false)
                    }),
            )
            .when(self.composer.read(cx).value().is_empty(), |field| {
                field.child(composer_hint(hint, hint_elapsed))
            })
    }

    /// The text grew (you typed): emit a particle from the caret. At most
    /// one every `PARTICLE_GAP` and `PARTICLES_MAX` alive.
    /// None when Configuration turned them off, or with reduced motion.
    pub(in crate::features::session) fn emit_typing_particle(&mut self, cx: &mut Context<Self>) {
        let len = self.composer.read(cx).value().len();
        let typed = len > self.composer_len;
        self.composer_len = len;
        if !typed || !crate::shared::kisses::typing(cx).plays(cx.reduce_motion()) {
            return;
        }
        self.typing_particles
            .retain(|(at, _, _)| at.elapsed() < PARTICLE_LIFE);
        let recent = self
            .typing_particles
            .last()
            .is_some_and(|(at, _, _)| at.elapsed() < PARTICLE_GAP);
        if recent || self.typing_particles.len() >= PARTICLES_MAX {
            return;
        }
        let Some(caret) = self.composer.read(cx).prepaint_caret_geometry() else {
            return;
        };
        let bounds = caret.cursor_bounds();
        let at = point(bounds.origin.x, bounds.origin.y);
        self.typing_particles
            .push((Instant::now(), at, self.particles_emitted));
        self.particles_emitted += 1;
    }

    /// Drop expired particles and end a finished focus pulse; whether the
    /// pulse still plays (particles ask for frames on the live layer instead).
    pub(super) fn composer_animating(&mut self) -> bool {
        self.typing_particles
            .retain(|(at, _, _)| at.elapsed() < PARTICLE_LIFE);
        if self.composer_pulse.is_some_and(|at| at.elapsed() >= PULSE) {
            self.composer_pulse = None;
        }
        self.composer_pulse.is_some()
    }

    /// Paws and notes rise from the caret, swaying and fading. While any
    /// are in flight, only the live layer asks for its next frame; the
    /// conversation is reused from its cache.
    fn render_typing_particles(&self) -> Option<Div> {
        if self.typing_particles.is_empty() {
            return None;
        }
        let particles = self.typing_particles.clone();
        let build = move || {
            let particles = particles.clone();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let mut flying = false;
                    for (born, from, i) in &particles {
                        let age = born.elapsed().as_secs_f32() / PARTICLE_LIFE.as_secs_f32();
                        if age >= 1. {
                            continue;
                        }
                        flying = true;
                        let i = *i;
                        let travel = 1. - (1. - age).powi(3);
                        let rise = 26. + 18. * noise(i, 2);
                        let drift = (noise(i, 1) * 2. - 1.) * 12.;
                        let sway = (age * 9. + i as f32).sin() * 4.;
                        let at = point(
                            from.x + px(drift * travel + sway),
                            from.y - px(rise * travel),
                        );
                        let grow = ui::spring((age * 3.).min(1.));
                        let dot = (1.1 + 0.4 * noise(i, 3)) * (0.5 + 0.5 * grow);
                        let alpha = if age > 0.55 { (1. - age) / 0.45 } else { 1. };
                        paint_particle(window, i, at, dot, alpha);
                    }
                    // Only the layer's copy paints: the slot's is invisible.
                    if flying {
                        ui::request_animation_frame(window);
                    }
                },
            )
            .absolute()
            .size_0()
            .into_any_element()
        };
        Some(
            self.live_slots
                .slot("typing-particles", build)
                .absolute()
                .size_0(),
        )
    }

    /// Play the focus pulse (none with reduced motion): frames are asked
    /// for until it's over ([`Self::composer_animating`]). Timed from
    /// `composer_pulse` rather than `with_animation`, whose progress is
    /// lost when the session view is reused from cache, which would replay
    /// the pulse on every later redraw.
    pub(in crate::features::session) fn start_composer_pulse(&mut self, cx: &mut Context<Self>) {
        if !cx.reduce_motion() {
            self.composer_pulse = Some(Instant::now());
        }
    }

    /// The composer's box (background, border, shadow), drawn as a layer
    /// behind its content. During the focus pulse it swells outward on
    /// every side and settles back along one smooth arc. Only this layer
    /// moves (it's absolute), so the text, the buttons and everything
    /// around stay put, and nothing stays bigger.
    fn composer_shell(&self) -> Div {
        /// How far the pulse reaches past the box, on every side.
        const REACH: f32 = 6.;
        let t = self
            .composer_pulse
            .map(|at| (at.elapsed().as_secs_f32() / PULSE.as_secs_f32()).min(1.))
            .unwrap_or(1.);
        let out = px(-REACH * (std::f32::consts::PI * ease_in_out(t)).sin());
        div()
            .absolute()
            .top(out)
            .bottom(out)
            .left(out)
            .right(out)
            .rounded_2xl()
            .border_1()
            .border_color(if self.composer_focused {
                ui::petal().opacity(0.45)
            } else {
                ui::border()
            })
            .bg(ui::surface())
            .shadow_lg()
    }

    /// The composer in the chosen input style, and for Prompt the status
    /// strip the pane draws along its bottom edge, edge to edge.
    pub(super) fn render_composer(
        &self,
        rem_size: Pixels,
        hint_elapsed: Option<Duration>,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Option<AnyElement>) {
        let empty = self.composer.read(cx).value().trim().is_empty()
            && self.attachments.is_empty()
            && self.files.is_empty();
        let busy = self.busy();
        let ready = self.backend.is_ready();
        let style = crate::features::session::input_style::style(cx);
        let stop_now = self.stoppable() && empty;
        let send_tip = if busy {
            t("Steer the running agent")
        } else {
            t("Send")
        };

        let action_button = match style {
            InputStyle::Bar => self.bar_action(stop_now, empty || !ready, send_tip, cx),
            InputStyle::Prompt => self.console_action(stop_now, empty || !ready, send_tip, cx),
            InputStyle::Soft => self.soft_action(stop_now, empty || !ready, send_tip, cx),
            _ if stop_now => Button::new("stop")
                .small()
                .danger()
                .rounded(px(999.))
                .child(div().size(rems(9. / 16.)).rounded_sm().bg(ui::well()))
                .tooltip(t("Stop ({shortcut})").replace("{shortcut}", ui::shortcut(".").as_ref()))
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                .into_any_element(),
            _ => Button::new("send")
                .small()
                .primary()
                .rounded(px(999.))
                .icon(Icon::new(IconName::ArrowUp))
                .tooltip(send_tip)
                .disabled(empty || !ready)
                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
                .into_any_element(),
        };

        let width = self.pane_width.get();
        let narrow = width < f32::from(rems(NARROW / 16.).to_pixels(rem_size));
        // The activity line folds every extension status (and the legacy
        // limit chips) into one chip below `TINY`.
        let collapse = width < f32::from(rems(TINY / 16.).to_pixels(rem_size));
        // When the legacy limit chips are on the activity line, the plan
        // chip has nothing to add: one number, never the same percent twice.
        let legacy_chips_shown = !collapse && !self.extension.rate_limits.is_empty();
        let attach = Button::new("attach")
            .ghost()
            .xsmall()
            .text_color(ui::muted())
            // Soft opens with a +, as its round controls do.
            .icon(Icon::new(if style == InputStyle::Soft {
                IconName::Plus
            } else {
                IconName::Paperclip
            }))
            .tooltip(t("Attach files or images (or paste / drop them)"))
            .disabled(!ready)
            .on_click(cx.listener(|this, _, window, cx| this.pick_attachments(window, cx)));
        // The model, effort and plan chips every style keeps together.
        let controls = h_flex()
            .min_w_0()
            .flex_shrink(1.)
            .gap_1p5()
            .items_center()
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .overflow_hidden()
                    .child(self.render_model_picker(narrow, cx)),
            )
            .when(
                self.models.current.as_ref().is_some_and(|m| m.reasoning),
                |this| this.child(self.render_effort_picker(collapse, cx)),
            )
            // The plan-limits chip keeps its width; the model picker before
            // it is the one that shrinks in a narrow pane.
            .when_some(
                self.render_plan_limits_chip(legacy_chips_shown, cx),
                |this, chip| this.child(div().flex_shrink_0().child(chip)),
            );

        // What the session spent, quiet beside the send; a narrow pane
        // leaves it out.
        let usage = (!narrow)
            .then(|| self.render_usage_chip())
            .flatten()
            .map(|chip| {
                div()
                    .debug_selector(|| "context-usage-chip".into())
                    .flex_shrink_0()
                    .child(chip)
            });
        let ring = self.usage.context_visible(ready).then(|| {
            div()
                .id("context-ring")
                .debug_selector(|| "context-ring".into())
                .relative()
                .flex_shrink_0()
                .child(self.render_context_control(cx))
                .when(self.usage.compacting, |ring| {
                    ring.child(
                        self.live_slots
                            .slot("context-compaction-orbit", || {
                                ui::paused_orbit(ui::info(), rems(99.)).into_any_element()
                            })
                            .debug_selector(|| "context-compaction-orbit".into())
                            .absolute()
                            .top(px(-2.))
                            .left(px(-2.))
                            .right(px(-2.))
                            .bottom(px(-2.)),
                    )
                })
        });
        let action = div()
            .relative()
            .flex_shrink_0()
            .child(action_button)
            .children(self.action_ring(style));
        let spacer = || div().flex_1().min_w_0();
        let row = || h_flex().w_full().min_w_0().gap_1p5().items_center();
        // The line or frame a focused composer lights up.
        let edge = |rest: Hsla| {
            if self.composer_focused {
                ui::petal().opacity(0.45)
            } else {
                rest
            }
        };
        let attachments = self.render_attachments(cx);
        let text = self.render_text_field(ready, hint_elapsed, cx);
        let pad = |d: Div| if narrow { d.px_3() } else { d.px_4() };

        let mut strip = None;
        let composer = match style {
            // A floating card, its controls inside.
            InputStyle::Card => v_flex().w_full().min_w_0().gap_1p5().child(
                div()
                    .relative()
                    .w_full()
                    .min_w_0()
                    .child(self.composer_shell())
                    .child(
                        pad(v_flex())
                            .relative()
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .gap_2()
                            .pt_4()
                            .pb_2p5()
                            .children(attachments)
                            .child(text)
                            .child(
                                row()
                                    .child(attach)
                                    .child(controls)
                                    .child(spacer())
                                    .children(ring)
                                    .children(usage)
                                    .child(action),
                            ),
                    ),
            ),
            // Spine: the pane's dock draws the strip and its hairline (see
            // `Render for SessionView`); here the plain text, then one row:
            // flat model and effort chips, a divider, attach, and a
            // keycap send or a Stop · esc button at the end.
            InputStyle::Bar => v_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .children(attachments)
                .child(div().px_1().min_h(rems(28. / 16.)).child(text))
                .child(
                    row()
                        .gap_1()
                        .child(controls)
                        .child(
                            div()
                                .flex_none()
                                .w(px(1.))
                                .h(rems(14. / 16.))
                                .mx_1()
                                .bg(ui::border()),
                        )
                        .child(attach)
                        .child(spacer())
                        .children(ring)
                        .children(usage)
                        .child(action),
                ),
            // Console: a framed `❯` line in the code font; the model,
            // spend and keys move to a status strip along the pane's
            // bottom edge, as a terminal's status line.
            InputStyle::Prompt => {
                let segment = |d: Div| {
                    d.h_full()
                        .flex()
                        .items_center()
                        .flex_none()
                        .px_2p5()
                        .border_r_1()
                        .border_color(ui::border())
                };
                strip = Some(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .h(rems(26. / 16.))
                        .flex_none()
                        .border_t_1()
                        .border_color(ui::border())
                        .bg(ui::well())
                        // Its bottom corners follow the pane's.
                        .rounded_b_xl()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(12.))
                        .text_color(ui::faint())
                        .overflow_hidden()
                        .child(
                            segment(div())
                                .rounded_bl_xl()
                                .bg(ui::accent().opacity(0.16))
                                .child(paw(ui::code_rems(12.), ui::accent())),
                        )
                        .child(segment(div()).min_w_0().flex_shrink(1.).child(controls))
                        .child(segment(div()).px_1().child(attach))
                        .child(spacer())
                        .children(ring)
                        .children(usage.map(|usage| {
                            segment(div()).px_1().border_r_0().border_l_1().child(usage)
                        }))
                        .child(segment(div()).border_r_0().border_l_1().child(action))
                        .into_any_element(),
                );
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_start()
                    .pl_3()
                    .pr_1()
                    .py(rems(2. / 16.))
                    .relative()
                    .rounded_md()
                    .border_1()
                    .border_color(edge(ui::border_strong()))
                    .bg(ui::well())
                    .font_family(ui::mono())
                    // On the text's first line: the text area's own top
                    // padding, then a line as tall as its lines.
                    .child(
                        div()
                            .flex_none()
                            .pt(TEXT_PAD)
                            .text_size(rems(14. / 16.))
                            .line_height(relative(TEXT_LINE))
                            .text_color(ui::accent())
                            .child("❯"),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .children(attachments)
                            .child(text),
                    )
            }
            // One rounded pill: attach, text and send on a line; the
            // model and spend in a quiet row below.
            InputStyle::Pill => v_flex()
                .w_full()
                .min_w_0()
                .gap_1p5()
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .items_end()
                        .gap_1()
                        .px_1p5()
                        .relative()
                        .rounded(rems(22. / 16.))
                        .border_1()
                        .border_color(edge(ui::border()))
                        .bg(ui::surface())
                        // Attach and send sit centered on the last line of
                        // text: each in a box as tall as one line with the
                        // text area's padding.
                        .child(on_text_line(attach))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .children(attachments.map(|a| div().pt_2().child(a)))
                                .child(text),
                        )
                        .child(on_text_line(action)),
                )
                .child(
                    row()
                        .px_2()
                        .child(controls)
                        .child(spacer())
                        .children(ring)
                        .children(usage),
                ),
            // Soft (after Devin's): a rounded box, the text on top and a
            // row of quiet round controls under it, the spend, the model
            // and a filled send at its end.
            InputStyle::Soft => v_flex().w_full().min_w_0().gap_1p5().child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .px_2()
                    .pt_1p5()
                    .pb_2()
                    .gap_1()
                    .relative()
                    .rounded(rems(20. / 16.))
                    .border_1()
                    .border_color(edge(ui::border()))
                    .bg(ui::surface())
                    .children(attachments.map(|a| div().px_2p5().pt_2().child(a)))
                    .child(div().min_h(rems(56. / 16.)).child(text))
                    .child(
                        row()
                            .px_0p5()
                            .child(attach)
                            .child(spacer())
                            .children(ring)
                            .children(usage)
                            .child(controls)
                            .child(action),
                    ),
            ),
        };

        let main = v_flex()
            .w_full()
            .min_w_0()
            .max_w(rems(720. / 16.))
            .gap_1p5()
            .children(self.render_slash_menu(cx))
            .children(self.render_dialog(cx))
            .children(self.render_review_confirm(cx))
            .child(self.render_activity(rem_size, cx))
            .child(composer)
            .children(self.render_typing_particles());
        (main.into_any_element(), strip)
    }

    /// Spine's keys: Stop as a framed button that names its key, send as a
    /// small ↵ keycap.
    fn bar_action(
        &self,
        stop: bool,
        blocked: bool,
        send_tip: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if stop {
            return h_flex()
                .id("stop")
                .h(rems(28. / 16.))
                .px_2p5()
                .gap_1p5()
                .rounded_md()
                .bg(ui::surface())
                .border_1()
                .border_color(ui::border_strong())
                .cursor_pointer()
                .hover(|s| s.bg(ui::surface_hover()))
                .tooltip(move |window, cx| {
                    Tooltip::new(
                        t("Stop (esc · {shortcut})")
                            .replace("{shortcut}", ui::shortcut(".").as_ref()),
                    )
                    .build(window, cx)
                })
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                .child(div().size(rems(8. / 16.)).rounded_sm().bg(ui::fg()))
                .child(
                    div()
                        .text_size(rems(12. / 16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg())
                        .child(t("Stop")),
                )
                .child(
                    div()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(11.))
                        .text_color(ui::faint())
                        .child("esc"),
                )
                .into_any_element();
        }
        h_flex()
            .id("send")
            .h(rems(26. / 16.))
            .min_w(rems(30. / 16.))
            .px_2()
            .justify_center()
            .rounded_md()
            .bg(ui::surface())
            .border_1()
            .border_color(ui::border_strong())
            .text_size(rems(12. / 16.))
            .tooltip(move |window, cx| Tooltip::new(send_tip).build(window, cx))
            .map(|d| {
                if blocked {
                    d.text_color(ui::faint())
                } else {
                    d.text_color(ui::muted())
                        .cursor_pointer()
                        .hover(|s| s.bg(ui::surface_hover()).text_color(ui::fg()))
                        .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
                }
            })
            .child("↵")
            .into_any_element()
    }

    /// Console's keys, as a terminal's status line names them: `esc stop`
    /// while the agent runs, else `↵ send`.
    fn console_action(
        &self,
        stop: bool,
        blocked: bool,
        send_tip: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (key, word) = if stop {
            ("esc", "stop")
        } else {
            ("↵", "send")
        };
        h_flex()
            .id(if stop { "stop" } else { "send" })
            .gap_1()
            .child(div().text_color(ui::muted()).child(key))
            .child(word)
            .map(|d| {
                if stop {
                    d.cursor_pointer()
                        .hover(|s| s.text_color(ui::danger()))
                        .tooltip(|window, cx| Tooltip::new(t("Stop the agent")).build(window, cx))
                        .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                } else if blocked {
                    d.opacity(0.6)
                } else {
                    d.cursor_pointer()
                        .hover(|s| s.text_color(ui::fg()))
                        .tooltip(move |window, cx| Tooltip::new(send_tip).build(window, cx))
                        .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
                }
            })
            .into_any_element()
    }

    /// Soft's send: a filled round button in the text color (the page
    /// color for its arrow), dimmed until there's something to send; a
    /// square while the agent runs stops it.
    fn soft_action(
        &self,
        stop: bool,
        blocked: bool,
        send_tip: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = h_flex()
            .id(if stop { "stop" } else { "send" })
            .size(rems(28. / 16.))
            .flex_none()
            .justify_center()
            .rounded_full()
            .bg(ui::fg());
        if stop {
            return button
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .tooltip(move |window, cx| {
                    Tooltip::new(
                        t("Stop ({shortcut})").replace("{shortcut}", ui::shortcut(".").as_ref()),
                    )
                    .build(window, cx)
                })
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                .child(div().size(rems(9. / 16.)).rounded_sm().bg(ui::screen_bg()))
                .into_any_element();
        }
        button
            .tooltip(move |window, cx| Tooltip::new(send_tip).build(window, cx))
            .map(|d| {
                if blocked {
                    d.opacity(0.3)
                } else {
                    d.cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
                }
            })
            .child(
                Icon::new(IconName::ArrowUp)
                    .small()
                    .text_color(ui::screen_bg()),
            )
            .into_any_element()
    }
}

/// The composer text area's own padding above and below its text
/// (gpui-component's multi-line padding at the default size).
const TEXT_PAD: Pixels = px(8.);
/// The text area's line height, relative to its 14 px text.
const TEXT_LINE: f32 = 1.5;

/// `element` centered on one line of the composer's text, padding
/// included: beside a text area it lines up with its line.
fn on_text_line(element: impl IntoElement) -> Div {
    div().flex_none().py(TEXT_PAD).child(
        div()
            .h(rems(14. * TEXT_LINE / 16.))
            .flex()
            .items_center()
            .child(element),
    )
}

/// The × in an attachment's top-right corner.
fn remove_button(
    id: impl Into<ElementId>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .absolute()
        .top(rems(3. / 16.))
        .right(rems(3. / 16.))
        .size(rems(18. / 16.))
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(ui::well())
        .cursor_pointer()
        .hover(|s| s.bg(ui::danger()))
        .tooltip(|window, cx| Tooltip::new(t("Remove")).build(window, cx))
        .on_click(on_click)
        .child(Icon::new(IconName::X).xsmall().text_color(ui::fg()))
}

/// Pasted text `n` waiting to be sent: a page with its first lines in tiny
/// type, then `#N` and how many lines it has.
fn pasted_card(n: usize, paste: &PastedText) -> Div {
    let lines = match paste.lines {
        1 => t("+1 line").to_string(),
        lines => t("+{count} lines").replace("{count}", &lines.to_string()),
    };
    h_flex()
        .relative()
        .flex_shrink_0()
        .h(rems(64. / 16.))
        .pl(rems(7. / 16.))
        .pr(rems(30. / 16.))
        .gap_2p5()
        .rounded_lg()
        .border_1()
        .border_color(ui::border_strong())
        .bg(ui::surface())
        .child(
            div()
                .flex_shrink_0()
                .w(rems(38. / 16.))
                .h(rems(48. / 16.))
                .p(rems(4. / 16.))
                .rounded_md()
                .overflow_hidden()
                .border_1()
                .border_color(ui::border())
                .bg(ui::well())
                .font_family(ui::mono())
                .text_size(ui::code_rems(5.))
                .line_height(ui::code_rems(6.))
                .text_color(ui::faint())
                .whitespace_nowrap()
                .child(paste.preview.clone()),
        )
        .child(
            v_flex()
                .gap_0p5()
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Icon::new(IconName::FileText)
                                .xsmall()
                                .text_color(ui::muted()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(ui::fg())
                                .child(format!("#{n}")),
                        ),
                )
                .child(
                    div()
                        .text_size(rems(11. / 16.))
                        .text_color(ui::muted())
                        .child(lines),
                ),
        )
}
