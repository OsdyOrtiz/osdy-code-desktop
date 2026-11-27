//! Pixel sprites painted dot by dot: paws and notes for the welcome and
//! composer flights, a heart for the finale stamp, and a shared painter
//! for the hydration drops.

use gentle_core::typing_kisses::{self, TypingKisses};
use gpui_kit::*;

use crate::shared::themes;

struct TypingChoice(TypingKisses);

impl Global for TypingChoice {}

/// Read the saved choice for the composer's typing particles.
pub fn init(cx: &mut App) {
    let saved = themes::store_path().and_then(|path| themes::read_key(&path, typing_kisses::KEY));
    cx.set_global(TypingChoice(TypingKisses::from_json(saved.as_ref())));
}

/// The composer's typing particles as chosen in Configuration.
pub fn typing(cx: &App) -> TypingKisses {
    cx.try_global::<TypingChoice>()
        .map_or_else(TypingKisses::default, |c| c.0)
}

/// Choose and save whether the composer shows typing particles.
pub fn set_typing(choice: TypingKisses, cx: &mut App) {
    cx.set_global(TypingChoice(choice));
    if let Some(path) = themes::store_path()
        && let Err(err) = themes::write_key(&path, typing_kisses::KEY, choice.to_json())
    {
        eprintln!("gentle-shell-gpui: could not save the typing paws & notes setting: {err}");
    }
}

// In these grids, # is the fill, o the darker edge, and . is transparent.
const PAW: [&str; 8] = [
    "..oo.oo..",
    "..##.##..",
    "oo.....oo",
    "#o..o..o#",
    "...o#o...",
    "..o###o..",
    ".o#####o.",
    "..ooooo..",
];
const EIGHTH_NOTE: [&str; 8] = [
    "...o..", "...#o.", "...##o", "...#.o", "...#..", ".oo#..", "o###..", ".oo...",
];
const BEAMED_NOTES: [&str; 8] = [
    ".....oooo",
    "..o######",
    "..###o..#",
    "..#.....#",
    "..#...oo#",
    "oo#..o###",
    "###...oo.",
    ".oo......",
];
/// The welcome finale's heart stamp.
pub const HEART: [&str; 6] = [
    ".##.##.", "#######", "#######", ".#####.", "..###..", "...#...",
];

fn particle(i: usize) -> (&'static [&'static str], u32, u32) {
    if i.is_multiple_of(2) {
        (&PAW, 0xbb9af7, 0x7652a4)
    } else {
        let sprite = if i % 4 == 1 {
            &EIGHTH_NOTE[..]
        } else {
            &BEAMED_NOTES[..]
        };
        (sprite, 0x7dcfff, 0x386b92)
    }
}

/// Paint the shared flight mix: a paw every other particle, with two note shapes.
pub fn paint_particle(window: &mut Window, i: usize, center: Point<Pixels>, dot: f32, alpha: f32) {
    let (sprite, body, line) = particle(i);
    let body: Hsla = rgb(body).into();
    let line: Hsla = rgb(line).into();
    paint_sprite(
        window,
        sprite,
        center,
        dot,
        body.opacity(alpha),
        line.opacity(alpha),
    );
}

/// A stable pseudo-random value in 0..1 for particle `i`'s `salt`.
pub fn noise(i: usize, salt: u32) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.
}

/// Paint `sprite` centered on `center`, `dot` px per pixel.
pub fn paint_sprite(
    window: &mut Window,
    sprite: &[&str],
    center: Point<Pixels>,
    dot: f32,
    body: Hsla,
    line: Hsla,
) {
    let rows = sprite.len() as f32;
    let cols = sprite[0].len() as f32;
    let left = center.x - px(cols * dot / 2.);
    let top = center.y - px(rows * dot / 2.);
    for (y, row) in sprite.iter().enumerate() {
        for (x, cell) in row.chars().enumerate() {
            let color = match cell {
                '#' => body,
                'o' => line,
                _ => continue,
            };
            let origin = point(left + px(x as f32 * dot), top + px(y as f32 * dot));
            window.paint_quad(fill(
                Bounds::new(origin, size(px(dot * 0.9), px(dot * 0.9))),
                color,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HEART, particle};

    fn particle_sprite(i: usize) -> &'static [&'static str] {
        particle(i).0
    }

    #[test]
    fn flight_selection_alternates_paws_and_notes_deterministically() {
        let paw = particle_sprite(0);
        let run: Vec<_> = (0..64).map(particle).collect();
        assert_eq!(run, (0..64).map(particle).collect::<Vec<_>>());
        for (i, (sprite, body, _)) in run.into_iter().enumerate() {
            let even = i.is_multiple_of(2);
            assert_eq!(sprite == paw, even);
            assert_eq!(body, if even { 0xbb9af7 } else { 0x7dcfff });
        }
    }

    #[test]
    fn particle_grids_include_a_paw_and_two_well_formed_notes() {
        let sprites: std::collections::HashSet<_> = (0..64).map(particle_sprite).collect();
        assert_eq!(sprites.len(), 3);
        let mut widths: Vec<_> = sprites.iter().map(|rows| rows[0].len()).collect();
        widths.sort_unstable();
        assert_eq!(widths, [6, 9, 9]);
        assert!(sprites.iter().all(|sprite| sprite.len() == 8));
        for sprite in sprites.into_iter().chain([HEART.as_slice()]) {
            assert!(sprite.iter().all(|row| row.len() == sprite[0].len()));
            assert!(
                sprite
                    .iter()
                    .flat_map(|row| row.bytes())
                    .all(|cell| matches!(cell, b'#' | b'o' | b'.' | b' '))
            );
        }
    }
}
