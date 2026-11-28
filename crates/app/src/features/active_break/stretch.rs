//! A seated figure stands up and stretches. Only the sprite changes frames.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::shared::{i18n::t, kisses::paint_sprite, ui};

use super::DURATION;

const SEATED: [&str; 13] = [
    ".............",
    ".............",
    ".............",
    ".....oo......",
    ".....oo......",
    "...#.##......",
    "...#.####....",
    "...#.##......",
    "...#.####....",
    "...####.#....",
    "....#...#....",
    "....#...##...",
    ".............",
];
const RISING: [&str; 13] = [
    ".............",
    ".............",
    "......oo.....",
    "......oo.....",
    ".....###.....",
    "....#######..",
    ".....##......",
    ".....###.....",
    "......#.#....",
    "......#.#....",
    ".....#..#....",
    ".....##.##...",
    ".............",
];
const STRETCHING: [&str; 13] = [
    "...#.....#...",
    "...#.....#...",
    "....#oo.#....",
    "....#oo.#....",
    ".....###.....",
    ".....###.....",
    ".....###.....",
    ".....###.....",
    ".....#.#.....",
    ".....#.#.....",
    ".....#.#.....",
    "....##.##....",
    ".............",
];

pub fn overlay(run: u64, cx: &App) -> AnyElement {
    if cx.reduce_motion() {
        return card(&STRETCHING).into_any_element();
    }
    // The animation wrapper has a fresh identity even when replayed mid-run.
    div()
        .with_animation(
            ElementId::NamedInteger("active-break-reminder".into(), run),
            Animation::new(DURATION),
            |el, progress| {
                let pose = if progress < 0.12 {
                    &SEATED
                } else if progress < 0.23 {
                    &RISING
                } else {
                    &STRETCHING
                };
                el.child(card(pose))
            },
        )
        .into_any_element()
}

fn card(pose: &'static [&'static str]) -> Div {
    h_flex()
        .gap_4()
        .px_5()
        .py_3()
        .rounded_2xl()
        .bg(ui::popover_bg())
        .border_1()
        .border_color(ui::border_strong())
        .shadow_2xl()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    paint_sprite(window, pose, bounds.center(), 3., ui::accent(), ui::fg());
                },
            )
            .flex_none()
            .size(px(39.)),
        )
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ui::fg())
                        .child(t("Time for an active break")),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(ui::muted())
                        .child(t("Stand up, stretch, and clear your head.")),
                ),
        )
}
