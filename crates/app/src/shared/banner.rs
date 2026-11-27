//! The startup banner and the legacy braille rose used by onboarding and surge.

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

const ROSE: &[&str] = &[
    "            ⣠⣿⣿⣶⣶⣤⣤⣤⣤⣤⣤⣤⣀    ⢀⡀",
    "         ⢀⣴⣾⣿⣿⠟⠝⢙⣙⣙⣙⣙⢙⣿⣷⢿⢿⢿⢿⢿⣿⣷⣄⡀",
    "      ⣀⣠⣴⣿⣿⣿⠟⢡⣶⡿⠟⢟⡽⣿⣿⣿⣿⡟⢶⣤⣤⣄⣁⠁⠙⢿⣿⣦",
    "   ⢀⣴⣿⣿⠅⣿⣿⣿⠅⢀⣿⠏ ⢰⡏⢹⢯⡤⢖⣯⣍⠻⡍⢻⡌⢽⣷⠄ ⢿⣿⡷⢶⣤⣤",
    "⢀⣀⣴⣿⣿⣿⠟ ⠹⣿⣿  ⢻⣇ ⠘⠻⣦⣈⢙⣻⣿⣷⣾⡥⠜⢁⣼⠟⠁⢴⣿⡿⠁⢽⣿⣿⣄",
    "⢻⣿⢿⣿⣿⣟⠅  ⠘⢿⣧ ⠘⠙⢦⣀  ⠙⠳⢤  ⢀⡴⠞⠛⠝⢀⣴⣿⠟  ⢽⣿⣿⢿⡆",
    " ⣿⢝⢿⣿⣧⠁   ⢸⠍⠳⣄  ⠉⠓⠦⢄⢀⣀⣁⣤⣥⣤⣤⣶⠿⢿⡟⠁   ⣽⣿⢿⣽⡇",
    " ⢿⣟⠝⢿⣿⣷⣄  ⠝⠕⠅⠈⠂ ⢀⡤⠖⠛⠉⠉⡁⣀   ⢀⠅⢽⠇  ⢀⣽⣿⣿⣿⠏",
    " ⠈⠻⢷⣝⣝⣿⢿⢷⣤⣄    ⠴⠉⠄⢔⢴⢾⢿⢿⢿⢗⠔⠔⠄⠁ ⢁⢄⣵⣿⡿⠟⢉⢽⣷⡄",
    "    ⠈⠉⠉⢿⡛⠿⢿⣿⣶⣤⣀⡁  ⠁⠁⠙⠅⠙⠁⠁⠁ ⣀⣴⣾⢿⠿⠛⠉ ⢤⣽⣿⣿⣇",
    "       ⠈⢿⣧⡄ ⠁⠉⠛⠿⠿⢷⣶⣤⣀   ⢀⡴⠛⠉⠁ ⠁ ⢀⣴⢿⢿⣽⠟⠉⠁",
    "         ⠙⠻⣷⣥⣄⢀    ⠈⠙⢷⡄ ⠁ ⢀⢄⣴⣴⢵⠿⢟⣭⣽⠟⠁",
    "            ⢉⢙⣛⣿⠷⢤⣄⡀  ⢿⢠⠔⢓⣛⠿⠿⠷⠷⠟⠛⠉⠁",
    "            ⠈⠙⠛⠛⠓⠒⢖⠙⠷⡄⢈⡴⠟⢅⢴⣶⣤⣤⠄",
    "                   ⢷⡄⠱⢍⡤⠐⠉⠉⠉⠉",
    "                   ⠈⣷ ⠍",
    "                    ⣿⢄",
];

struct RoseArt {
    dots: Vec<(u16, u16)>,
    size: (u16, u16),
}

static ROSE_ART: LazyLock<RoseArt> = LazyLock::new(|| {
    // Braille U+2800: bits 0-2 and 6 are the left column (rows 0-3), 3-5 and 7 the right.
    const DOTS: [(u16, u16); 8] = [
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 1),
        (1, 2),
        (0, 3),
        (1, 3),
    ];
    let mut rose = Vec::new();
    let mut rose_w = 0;
    for (row, line) in ROSE.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let code = ch as u32;
            if !(0x2800..=0x28FF).contains(&code) {
                continue;
            }
            let bits = code - 0x2800;
            for (bit, (dx, dy)) in DOTS.iter().enumerate() {
                if bits & (1 << bit) != 0 {
                    rose.push((col as u16 * 2 + dx, row as u16 * 4 + dy));
                    rose_w = rose_w.max(col as u16 * 2 + dx + 1);
                }
            }
        }
    }
    RoseArt {
        dots: rose,
        size: (rose_w, ROSE.len() as u16 * 4),
    }
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerConfig {
    pub show_rose: bool,
    pub show_text_logo: bool,
}

impl Default for BannerConfig {
    fn default() -> Self {
        Self {
            show_rose: true,
            show_text_logo: true,
        }
    }
}

impl BannerConfig {
    /// Width in unscaled banner units, including the gap only when both layers show.
    pub fn width(self) -> f32 {
        match (self.show_rose, self.show_text_logo) {
            (true, true) => BADGE + GAP + WORDMARK,
            (true, false) => BADGE,
            (false, true) => WORDMARK,
            (false, false) => 0.,
        }
    }

    /// Reserve the banner's layout without loading images or asking for frames.
    pub fn placeholder(self, scale: f32) -> impl IntoElement {
        let height = if self.show_rose {
            BADGE
        } else if self.show_text_logo {
            WORDMARK / WORDMARK_ASPECT
        } else {
            0.
        };
        div().w(px(self.width() * scale)).h(px(height * scale))
    }

    /// The badge's mouth, relative to the banner's center, in unscaled units.
    pub fn kiss_anchor(self) -> Option<(f32, f32)> {
        self.show_rose.then(|| {
            let x = BADGE / 2. - self.width() / 2.;
            // Mouth center is at (192, 266) in the 384 px source canvas.
            let y = (266. / 384. - 0.5) * BADGE;
            (x, y)
        })
    }
}

/// `<config>/banner.json`, tolerant like the launcher's own read: missing or
/// malformed fields fall back to the defaults. Its `color` picks the
/// terminal banner's palette and is ignored by the desktop.
pub fn read_config(config_home: &Path) -> BannerConfig {
    let default = BannerConfig::default();
    let Ok(raw) = std::fs::read_to_string(config_home.join("banner.json")) else {
        return default;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return default;
    };
    BannerConfig {
        show_rose: value["showRose"].as_bool().unwrap_or(default.show_rose),
        show_text_logo: value["showTextLogo"]
            .as_bool()
            .unwrap_or(default.show_text_logo),
    }
}

/// The rose's dots (x, y in dot units) and its size, for other effects that
/// draw it (the max-effort surge).
pub fn rose() -> (&'static [(u16, u16)], (u16, u16)) {
    (&ROSE_ART.dots, ROSE_ART.size)
}

/// Gentle Shell's pixel rose, `height` px tall, in `color`.
pub fn pixel_rose(height: f32, color: Hsla) -> impl IntoElement {
    let (dots, (width, rows)) = rose();
    let dot = height / rows.max(1) as f32;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let side = px((dot * 0.55).max(1.0));
            for &(x, y) in dots {
                let origin = point(
                    bounds.origin.x + px(x as f32 * dot),
                    bounds.origin.y + px(y as f32 * dot),
                );
                window.paint_quad(fill(Bounds::new(origin, size(side, side)), color));
            }
        },
    )
    .w(px(width as f32 * dot))
    .h(px(height))
}

/// Drive `reveal` linearly from 0 to 1 over this interval.
pub const WRITE: Duration = Duration::from_millis(2600);

const BADGE: f32 = 48.;
const GAP: f32 = 5.;
const WORDMARK: f32 = 122.;
const WORDMARK_ASPECT: f32 = 1130. / 229.;
// The cursor SVG retains the wordmark viewBox (left edge 4) and the source's
// glyph origin at x=1017.29. Move the entire layer left to meet the clip edge.
const CURSOR_AT: f32 = (1017.29 - 4.) / 1130.;
const CURSOR_PERIOD: Duration = Duration::from_secs(1);
const EYE_PERIOD: Duration = Duration::from_secs(5);

fn cursor_edge(reveal: f32) -> f32 {
    let t = reveal.clamp(0., 1.);
    CURSOR_AT * t * t * (3. - 2. * t)
}

fn cursor_on(seconds: f32) -> bool {
    seconds % CURSOR_PERIOD.as_secs_f32() < 0.5
}

fn eyes_closed(seconds: f32) -> bool {
    seconds % EYE_PERIOD.as_secs_f32() >= 4.85
}

fn badge(scale: f32, still: bool) -> AnyElement {
    let side = px(BADGE * scale);
    let path = if still {
        "brand/badge-still.png"
    } else {
        "brand/badge.webp"
    };
    let badge = div()
        .relative()
        .flex_none()
        .size(side)
        .child(img(path).id("badge-image").size_full());
    if still {
        return badge.into_any_element();
    }
    badge
        .with_animation(
            "banner-eyes",
            Animation::new(EYE_PERIOD).repeat(),
            |el, t| {
                // Use the same canvas as the badge. The mask covers the eye positions
                // across all WebP frames, so the loops don't need to share a clock.
                let visible = eyes_closed(t * EYE_PERIOD.as_secs_f32());
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .opacity(if visible { 1. } else { 0. })
                        .child(img("brand/eyes-closed.svg").size_full()),
                )
            },
        )
        .into_any_element()
}

fn wordmark(scale: f32, reveal: f32, still: bool) -> impl IntoElement {
    let width = px(WORDMARK * scale);
    let height = width / WORDMARK_ASPECT;
    let reveal = if still { 1. } else { reveal };
    let edge = cursor_edge(reveal);
    let cursor = div()
        .absolute()
        .top_0()
        .left(width * (edge - CURSOR_AT))
        .w(width)
        .h(height)
        .child(img("brand/cursor.svg").size_full());
    let cursor = if still || reveal < 1. {
        cursor.into_any_element()
    } else {
        cursor
            .with_animation(
                "banner-cursor",
                Animation::new(CURSOR_PERIOD).repeat(),
                |el, t| {
                    el.opacity(if cursor_on(t * CURSOR_PERIOD.as_secs_f32()) {
                        1.
                    } else {
                        0.
                    })
                },
            )
            .into_any_element()
    };
    div()
        .relative()
        .flex_none()
        .w(width)
        .h(height)
        .overflow_hidden()
        .child(
            div()
                .relative()
                .w(width * edge)
                .h(height)
                .overflow_hidden()
                .child(
                    img("brand/wordmark-text.svg")
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(width)
                        .h(height),
                ),
        )
        .child(cursor)
}

/// The full-color brand in every theme. Reduced motion shows the completed,
/// static art regardless of the caller's reveal progress.
pub fn banner(config: BannerConfig, scale: f32, reveal: f32) -> impl IntoElement {
    Banner {
        config,
        scale,
        reveal,
    }
}

#[derive(IntoElement)]
struct Banner {
    config: BannerConfig,
    scale: f32,
    reveal: f32,
}

impl RenderOnce for Banner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = cx.reduce_motion();
        div()
            .flex()
            .items_center()
            .gap(px(self.scale * GAP))
            .when(self.config.show_rose, |el| {
                el.child(badge(self.scale, still))
            })
            .when(self.config.show_text_logo, |el| {
                el.child(wordmark(self.scale, self.reveal, still))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{BannerConfig, read_config, rose};

    #[test]
    fn legacy_rose_still_decodes_braille_for_other_effects() {
        let (dots, (width, height)) = rose();
        // The first full braille cell at column 13 sets all eight dots.
        for x in 26..28 {
            for y in 0..4 {
                assert!(dots.contains(&(x, y)));
            }
        }
        assert_eq!(dots.iter().map(|(x, _)| *x).max().unwrap() + 1, width);
        assert!(dots.iter().all(|(_, y)| *y < height));
    }

    #[test]
    fn cursor_moves_with_the_clip_and_lands_on_the_svg_glyph() {
        let edge = super::cursor_edge;
        assert_eq!(edge(-0.1), 0.);
        assert!(0. < edge(0.25) && edge(0.25) < edge(0.5));
        assert!(edge(0.5) < edge(0.75) && edge(0.75) < edge(1.));
        assert!((edge(1.) - (1017.29 - 4.) / 1130.).abs() < 0.0001);
        assert_eq!(edge(1.1), edge(1.));
    }

    #[test]
    fn idle_blinks_are_steps_and_repeat() {
        use super::{cursor_on, eyes_closed};
        assert!(cursor_on(0.));
        assert!(cursor_on(0.49));
        assert!(!cursor_on(0.5));
        assert!(!cursor_on(0.99));
        assert!(cursor_on(1.));
        assert!(!eyes_closed(0.));
        assert!(!eyes_closed(4.84));
        assert!(eyes_closed(4.85));
        assert!(eyes_closed(4.99));
        assert!(!eyes_closed(5.));
        assert!(eyes_closed(9.9));
    }

    #[test]
    fn kisses_track_the_badge_when_the_wordmark_is_hidden() {
        let both = BannerConfig::default();
        let badge_only = BannerConfig {
            show_text_logo: false,
            ..both
        };
        let text_only = BannerConfig {
            show_rose: false,
            ..both
        };
        let (both_x, both_y) = both.kiss_anchor().unwrap();
        let (badge_x, badge_y) = badge_only.kiss_anchor().unwrap();
        assert_eq!(badge_x, 0.);
        assert_eq!(both_x + (both.width() - badge_only.width()) / 2., badge_x);
        assert_eq!(both_y, badge_y);
        assert!(badge_y > 0. && badge_y < badge_only.width() / 2.);
        assert_eq!(text_only.kiss_anchor(), None);
    }

    #[test]
    fn config_falls_back_per_field() {
        let dir = std::env::temp_dir().join(format!("gs-banner-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("banner.json");
        assert_eq!(read_config(&dir), BannerConfig::default());
        std::fs::write(&config_path, "not JSON").unwrap();
        assert_eq!(read_config(&dir), BannerConfig::default());
        std::fs::write(
            &config_path,
            r#"{"showRose":false,"showTextLogo":1,"color":"cyan"}"#,
        )
        .unwrap();
        assert_eq!(
            read_config(&dir),
            BannerConfig {
                show_rose: false,
                show_text_logo: true
            }
        );
        std::fs::write(&config_path, r#"{"showTextLogo":false}"#).unwrap();
        assert_eq!(
            read_config(&dir),
            BannerConfig {
                show_rose: true,
                show_text_logo: false
            }
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
