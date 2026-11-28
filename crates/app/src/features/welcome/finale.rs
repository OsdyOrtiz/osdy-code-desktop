//! The entrance after Continue: the wordmark writes itself in again, pixel
//! paws and notes float up, and the farewell types in before a heart lands.
//! The screen then dissolves into the app; the animation takes no input.

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::shared::banner::{BannerConfig, banner};
use crate::shared::kisses::{HEART, noise, paint_particle, paint_sprite};
use crate::shared::ui;

/// From Continue to the app.
pub(super) const FINALE: Duration = Duration::from_millis(7600);
/// The dissolve starts this far in (0..1): the last ~0.78 s, after the
/// farewell has stayed on screen a couple of seconds.
pub(super) const DISSOLVE: f32 = 0.897;

/// The box the banner and the flying sprites share.
const STAGE_W: f32 = 760.;
const STAGE_H: f32 = 300.;
const BANNER_SCALE: f32 = 2.6;
/// When the letters start writing again; the banner takes WRITE to reveal.
const LETTERS_AT: f32 = 0.25;
const PARTICLES_AT: f32 = 2.0;
const PARTICLE_GAP: f32 = 0.16;
const PARTICLE_LIFE: f32 = 1.7;
const PARTICLES: usize = 14;
const TITLE: &str = "Osdy Code:";
const SUBTITLE: &str = "Corazón de Junior, Conocimiento de Senior";
const TITLE_AT: f32 = 2.85;
const SUBTITLE_AT: f32 = 3.18;
const LETTER_GAP: f32 = 0.025;
const HEART_AT: f32 = 4.25;

fn reveal_prefix(text: &str, count: usize) -> &str {
    let end = text
        .char_indices()
        .nth(count)
        .map_or(text.len(), |(i, _)| i);
    &text[..end]
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

/// Paws and notes leave the raccoon's mouth, float up swaying, swell, and fade.
fn particles(anchor: (f32, f32)) -> impl IntoElement {
    let total = FINALE.as_secs_f32();
    div().absolute().inset_0().with_animation(
        "finale-particles",
        Animation::new(FINALE),
        move |el, t| {
            let now = t * total;
            el.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let center = bounds.center();
                        let mouth = point(
                            center.x + px(anchor.0 * BANNER_SCALE),
                            center.y + px(anchor.1 * BANNER_SCALE),
                        );
                        for i in 0..PARTICLES {
                            let born = PARTICLES_AT + i as f32 * PARTICLE_GAP;
                            let age = (now - born) / PARTICLE_LIFE;
                            if !(0. ..1.).contains(&age) {
                                continue;
                            }
                            // Mostly up and outward, alternating sides.
                            let side = if i % 2 == 0 { 1. } else { -1. };
                            let drift = side * (60. + 180. * noise(i, 1));
                            let rise = 160. + 160. * noise(i, 2);
                            let sway = (age * 9. + i as f32).sin() * 10.;
                            let travel = ease_out(age);
                            let at = point(
                                mouth.x + px((drift + sway) * travel),
                                mouth.y - px(rise * travel),
                            );
                            // Pops in, swells a little, fades at the end.
                            let grow = ui::spring((age * 3.).min(1.));
                            let dot = (2.2 + 1.6 * noise(i, 3)) * (0.4 + 0.6 * grow);
                            let alpha = if age > 0.7 { (1. - age) / 0.3 } else { 1. };
                            paint_particle(window, i, at, dot, alpha);
                        }
                    },
                )
                .size_full(),
            )
        },
    )
}

/// The banner writing itself in again, slowly.
fn letters(config: BannerConfig) -> impl IntoElement {
    let total = FINALE.as_secs_f32();
    div().flex().items_center().justify_center().with_animation(
        "finale-letters",
        Animation::new(FINALE),
        move |el, t| {
            let reveal = ((t * total - LETTERS_AT) / crate::shared::banner::WRITE.as_secs_f32())
                .clamp(0., 1.);
            el.child(banner(config, BANNER_SCALE, reveal))
        },
    )
}

/// A pixel heart lands after the last letter.
fn stamp() -> impl IntoElement {
    const DOT: f32 = 5.;
    let total = FINALE.as_secs_f32();
    div()
        .w(px(HEART[0].len() as f32 * DOT))
        .h(px(HEART.len() as f32 * DOT))
        .child(
            canvas(
                |_, _, _| {},
                |bounds, _, window, _| {
                    let pink: Hsla = rgb(0xf7768e).into();
                    paint_sprite(window, &HEART, bounds.center(), DOT, pink, pink);
                },
            )
            .size_full(),
        )
        .with_animation("finale-stamp", Animation::new(FINALE), move |el, t| {
            let local = ((t * total - HEART_AT) / 0.45).clamp(0., 1.);
            let s = ui::spring(local);
            el.opacity(local).relative().top(px((1. - s) * -28.))
        })
}

/// Reveal complete characters while keeping the line's position fixed.
fn line(id: &'static str, text: &'static str, at: f32, font_px: f32) -> impl IntoElement {
    let total = FINALE.as_secs_f32();
    div()
        .relative()
        .whitespace_nowrap()
        .text_size(px(font_px))
        .line_height(px(font_px * 1.2))
        .font_weight(FontWeight::EXTRA_BOLD)
        .text_color(ui::petal())
        .child(div().opacity(0.).child(text))
        .child(div().absolute().left_0().top_0().with_animation(
            id,
            Animation::new(FINALE),
            move |el, t| {
                let elapsed = t * total - at;
                let count = (elapsed / LETTER_GAP).floor() as usize;
                let s = ui::spring((elapsed / 0.5).clamp(0., 1.));
                el.top(px((1. - s) * 18.)).child(reveal_prefix(text, count))
            },
        ))
}

/// The whole entrance.
pub(super) fn render(config: BannerConfig) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_1()
        .child(
            div()
                .relative()
                .w(px(STAGE_W))
                .h(px(STAGE_H))
                .flex()
                .items_center()
                .justify_center()
                .child(letters(config))
                .children(config.particle_anchor().map(particles)),
        )
        .child(line("finale-title", TITLE, TITLE_AT, 50.))
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(line("finale-subtitle", SUBTITLE, SUBTITLE_AT, 26.))
                .child(stamp()),
        )
}

#[cfg(test)]
mod tests {
    use super::{SUBTITLE, TITLE, reveal_prefix};

    #[test]
    fn finale_reveal_keeps_every_prefix_on_a_char_boundary() {
        let phrase = format!("{TITLE} {SUBTITLE}");
        assert_eq!(
            phrase,
            "Osdy Code: Corazón de Junior, Conocimiento de Senior"
        );
        let mut expected = String::new();
        assert_eq!(reveal_prefix(&phrase, 0), expected);
        for (i, letter) in phrase.chars().enumerate() {
            expected.push(letter);
            assert_eq!(reveal_prefix(&phrase, i + 1), expected);
        }
        assert_eq!(reveal_prefix(&phrase, phrase.chars().count()), phrase);
    }
}
