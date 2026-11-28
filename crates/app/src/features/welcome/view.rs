//! Render: the full-window glow, animated brand banner, heading, install
//! sequence, and footer. ↵ is the main button.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{AFTER_INTRO, WelcomeView, finale};
use crate::shared::banner::{WRITE, banner, night};
use crate::shared::{i18n::t, ui};

/// The client-drawn title bar (Windows, Linux) keeps its window controls
/// visible above the screen; on macOS the screen fills the window under the
/// native traffic lights.
const TITLE_BAR: f32 = 36.;
/// The banner is ~175 px wide per unit of scale.
const BANNER_SCALE: f32 = 2.6;
/// The veil over an animated theme: everywhere, then a band behind the text.
const VEIL: f32 = 0.3;
const VEIL_BAND: f32 = 0.55;

impl WelcomeView {
    fn render_banner(&self, still: bool) -> AnyElement {
        let config = self.banner;
        if still {
            return banner(config, BANNER_SCALE, 1.).into_any_element();
        }
        // Keep the same element identity when the intro ends so the idle
        // animations don't restart. The ready state keeps its quieter glow.
        let intro_done = self.intro_done;
        div()
            .with_animation("welcome-banner", Animation::new(WRITE), move |el, t| {
                el.child(banner(
                    config,
                    BANNER_SCALE,
                    if intro_done { 1. } else { t },
                ))
            })
            .into_any_element()
    }

    fn render_heading(&self, cx: &App) -> AnyElement {
        let setup = self.setup.read(cx);
        let (title, line) = if setup.looking() {
            (
                "Welcome to Osdy Code Desktop.",
                "Looking at what this machine already has…",
            )
        } else if setup.ready() && setup.needs_sign_in() {
            (
                "You're ready.",
                "Everything Osdy Code Desktop needs is installed. Sign in to a model \
                 provider now, or later from the chat.",
            )
        } else if setup.ready() {
            (
                "You're ready.",
                "Everything Osdy Code Desktop needs is here. Open your first agent whenever \
                 you like; the extras below make it better.",
            )
        } else {
            (
                "Let's get you set up.",
                "A few steps and you're in. Everything installs right here; anything \
                 that needs your password opens in a terminal for you to run.",
            )
        };
        let heading = v_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(rems(34. / 16.))
                    .line_height(rems(42. / 16.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(night::text())
                    .child(t(title)),
            )
            .child(
                div()
                    .max_w(rems(560. / 16.))
                    .text_base()
                    .text_center()
                    .line_height(rems(24. / 16.))
                    .text_color(night::muted())
                    .child(t(line)),
            );
        let delay = if self.intro_done {
            std::time::Duration::ZERO
        } else {
            AFTER_INTRO
        };
        ui::appear_after("welcome-heading", delay, 10., heading).into_any_element()
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let setup = self.setup.read(cx);
        let busy = setup.busy();
        let ready = setup.ready();
        let checking = setup.checking();
        let can_install = setup.can_install();
        let (label, enabled) = if ready {
            ("Continue", !busy)
        } else if busy {
            ("Installing…", false)
        } else if can_install {
            ("Install everything", !checking)
        } else {
            // Only steps this screen can't run alone are left (sign in, a
            // terminal command): Continue waits for them.
            ("Continue", false)
        };
        // The brand blue in every theme, with night text on it.
        let blue = night::blue();
        let primary = Button::new("welcome-primary")
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(blue)
                    .foreground(night::bg())
                    .hover(blue.blend(white().opacity(0.12)))
                    .active(blue.blend(black().opacity(0.12))),
            )
            .large()
            .rounded(px(999.))
            .when(!ready && can_install, |b| {
                b.icon(Icon::new(IconName::ArrowDown))
            })
            .label(t(label))
            .loading(busy)
            .disabled(!enabled)
            .on_click(cx.listener(|this, _, _, cx| this.primary(cx)));
        let footer = h_flex()
            .flex_none()
            .h(rems(88. / 16.))
            .px_10()
            .gap_2()
            .when(!ready, |d| {
                d.child(
                    Button::new("welcome-skip")
                        .ghost()
                        .label(t("Skip for now"))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.enter(cx))),
                )
            })
            .child(
                Button::new("welcome-recheck")
                    .ghost()
                    .icon(Icon::new(IconName::RefreshCw))
                    .label(t("Check again"))
                    .loading(checking)
                    .disabled(busy || checking)
                    .on_click(cx.listener(|this, _, _, cx| this.check_again(cx))),
            )
            .child(div().flex_1())
            .child(primary);
        let delay = if self.intro_done {
            std::time::Duration::ZERO
        } else {
            AFTER_INTRO
        };
        ui::appear_after("welcome-footer", delay, 0., footer).into_any_element()
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "enter" {
            self.primary(cx);
            cx.stop_propagation();
        }
    }
}

impl Render for WelcomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.setup.read(cx).ready();
        let entrance = self.finale.is_some();
        let macos = cfg!(target_os = "macos");
        let root = div()
            .id("welcome")
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .top(rems(if macos { 0. } else { TITLE_BAR / 16. }))
            .occlude()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .bg(night::bg())
            // The theme's animated backdrop shows from the first launch,
            // under a see-through deep purple veil: light at the sides so the
            // scene always shows, denser behind the text column so it reads.
            .when_some(crate::shared::backdrop::layer(window, cx), |d, layer| {
                let plum = night::plum();
                let band = plum.opacity(VEIL_BAND);
                d.child(layer)
                    .child(div().absolute().inset_0().bg(plum.opacity(VEIL)))
                    .child(
                        h_flex()
                            .absolute()
                            .inset_0()
                            .child(div().flex_1().h_full().bg(linear_gradient(
                                90.,
                                linear_color_stop(plum.opacity(0.), 0.),
                                linear_color_stop(band, 1.),
                            )))
                            .child(div().flex_none().w(rems(760. / 16.)).h_full().bg(band))
                            .child(div().flex_1().h_full().bg(linear_gradient(
                                90.,
                                linear_color_stop(band, 0.),
                                linear_color_stop(plum.opacity(0.), 1.),
                            ))),
                    )
            })
            .child(
                // A soft brand-blue glow from the top: quieter once everything
                // required is ready, brighter for the entrance.
                div().absolute().inset_0().bg(linear_gradient(
                    180.,
                    linear_color_stop(
                        night::blue().opacity(if entrance {
                            0.18
                        } else if ready {
                            0.05
                        } else {
                            0.10
                        }),
                        0.,
                    ),
                    linear_color_stop(night::blue().opacity(0.), 0.6),
                )),
            );
        if entrance {
            // The entrance, then the screen dissolves into the app.
            return root
                .child(finale::render(self.banner))
                .with_animation(
                    "welcome-dissolve",
                    Animation::new(finale::FINALE),
                    |el, t| {
                        let fade = ((t - finale::DISSOLVE) / (1. - finale::DISSOLVE)).clamp(0., 1.);
                        // Smoothstep: eases into the fade and out of it.
                        el.opacity(1. - fade * fade * (3. - 2. * fade))
                    },
                )
                .into_any_element();
        }
        root.child(
            v_flex()
                .size_full()
                .when(macos, |d| {
                    // Under the traffic lights: drag the window from here.
                    d.child(
                        div()
                            .id("welcome-drag")
                            .flex_none()
                            .h(rems(TITLE_BAR / 16.))
                            .window_control_area(WindowControlArea::Drag)
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            }),
                    )
                })
                .child(
                    div()
                        .id("welcome-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(
                            v_flex()
                                .w_full()
                                .items_center()
                                .px_8()
                                .pt_6()
                                .pb_6()
                                .gap_6()
                                .child(self.render_banner(cx.reduce_motion()))
                                .child(self.render_heading(cx))
                                .child(
                                    div()
                                        .w_full()
                                        .max_w(rems(680. / 16.))
                                        .child(self.setup.clone()),
                                ),
                        ),
                )
                .child(self.render_footer(cx)),
        )
        .into_any_element()
    }
}
