//! Configuration › Appearance › Reading Desk: a collapsible specimen of the
//! fonts. It draws the applied fonts, or the family under the open picker's
//! cursor and any size being typed, only inside this panel: trying a font
//! here never applies, saves or re-renders anything else. The tiles follow the theme library's cards: a framed preview
//! in the theme's own terminal colors over a caption row.

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::SettingsView;
use super::fonts::effective_family;
use crate::shared::i18n::t;
use crate::shared::themes::{self, FontSlot, Fonts, Themes};
use crate::shared::ui;

/// Every tile has the same fixed bands, at 100 % zoom, so the three stay
/// aligned whatever family or size each one previews: the glyph band draws
/// its specimen at one fixed size (it shows the shape), and the sample band
/// draws the real size and clips what no longer fits instead of growing.
const GLYPH_BAND: f32 = 44.;
const GLYPH_SIZE: f32 = 26.;
const SAMPLE_BAND: f32 = 92.;

/// A theme color as an opaque fill.
fn solid(color: u32) -> Hsla {
    rgb(color).into()
}

/// One run of colored text inside a sample line.
fn run(text: &'static str, color: Hsla) -> Div {
    div().text_color(color).child(text)
}

fn line(runs: impl IntoIterator<Item = Div>) -> Div {
    h_flex()
        .whitespace_nowrap()
        .overflow_hidden()
        .children(runs)
}

/// Family, size and the tile's look for one slot.
struct Spec {
    slot: FontSlot,
    family: SharedString,
    size: Pixels,
    /// The size as the user typed it (at 100 % zoom), for the caption.
    points: f32,
    zoom: f32,
}

impl SettingsView {
    pub(super) fn reading_desk(&self, cx: &mut Context<Self>) -> Div {
        let open = self.fonts.desk_open;
        let (preview, previewing) = self.desk_fonts(cx);
        let header = h_flex()
            .id("reading-desk-toggle")
            .gap_2()
            .cursor_pointer()
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .xsmall()
                .text_color(ui::faint()),
            )
            .child(ui::section_label(t("Reading desk")))
            .child(
                div()
                    .ml_auto()
                    .text_xs()
                    .text_color(if previewing {
                        ui::accent()
                    } else {
                        ui::faint()
                    })
                    .child(if previewing {
                        t("Previewing · click or Enter to apply")
                    } else {
                        t("Live preview")
                    }),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.fonts.desk_open = !this.fonts.desk_open;
                cx.notify();
            }));
        let mut desk = v_flex().gap_2().pt_2().pb_3().child(header);
        if open {
            let interface = ui::default_interface(cx);
            let zoom = f32::from(cx.global::<Themes>().interface_size) / 100.;
            desk = desk.child(desk_tiles(&preview, &interface, zoom));
        }
        desk
    }
}

fn desk_tiles(fonts: &Fonts, default_interface: &SharedString, zoom: f32) -> Div {
    let spec = |slot| Spec {
        slot,
        family: effective_family(fonts, slot, default_interface),
        size: px(fonts.size(slot) * zoom),
        points: fonts.size(slot),
        zoom,
    };
    h_flex()
        .gap_3()
        .child(interface_tile(spec(FontSlot::Interface)))
        .child(code_tile(spec(FontSlot::Code)))
        .child(terminal_tile(spec(FontSlot::Terminal)))
}

/// The theme card's frame: a preview well over a caption naming the slot,
/// its family and its size.
fn tile(spec: &Spec, glyphs: &'static str, accessory: Option<Div>, preview: Div, bg: Hsla) -> Div {
    let zoom = spec.zoom;
    let caption = h_flex()
        .gap_1()
        .px(px(2.))
        .text_xs()
        .child(div().text_color(ui::muted()).child(t(match spec.slot {
            FontSlot::Interface => "Interface",
            FontSlot::Code => "Code",
            FontSlot::Terminal => "Terminal",
        })))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(ui::fg())
                .child(spec.family.clone()),
        )
        .child(
            div()
                .text_color(ui::faint())
                .font_family(ui::mono())
                .text_size(ui::code_rems(12.))
                // The interface text follows Interface size, not a size of its own.
                .child(if spec.slot.has_size() {
                    format!("{} px", trim(spec.points))
                } else {
                    format!("{:.0} %", spec.zoom * 100.)
                }),
        );
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_1p5()
        .p_1p5()
        .rounded_xl()
        .border_2()
        .border_color(ui::border())
        .child(
            v_flex()
                .px_2p5()
                .py_2()
                .rounded_lg()
                .bg(bg)
                .border_1()
                .border_color(ui::border().opacity(0.7))
                .overflow_hidden()
                .font_family(spec.family.clone())
                .child(
                    h_flex()
                        .h(px(GLYPH_BAND * zoom))
                        .flex_none()
                        .gap_2()
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(GLYPH_SIZE * zoom))
                                .line_height(px(GLYPH_BAND * zoom))
                                .text_color(ui::champagne())
                                .child(glyphs),
                        )
                        .children(accessory),
                )
                .child(
                    v_flex()
                        .h(px(SAMPLE_BAND * zoom))
                        .flex_none()
                        .overflow_hidden()
                        .text_size(spec.size)
                        .line_height(spec.size * 1.45)
                        .child(preview),
                ),
        )
        .child(caption)
}

/// `13` rather than `13.0`, `12.5` as is.
fn trim(size: f32) -> String {
    if size.fract() == 0. {
        format!("{size:.0}")
    } else {
        format!("{size:.1}")
    }
}

fn interface_tile(spec: Spec) -> Div {
    let preview = v_flex()
        .child(line([run(t("Review the latest changes"), ui::fg())]))
        .child(line([run(
            t("Résumé ready · 3 files · café, acción, 0123"),
            ui::muted(),
        )]))
        .child(
            h_flex().pt_1().child(
                div()
                    .px_2()
                    .rounded_md()
                    .bg(ui::accent().opacity(0.16))
                    .text_color(ui::accent())
                    .child(t("View changes")),
            ),
        );
    tile(&spec, "Aa Éé Ññ", None, preview, ui::raised())
}

fn code_tile(spec: Spec) -> Div {
    let t = themes::term(|t| *t);
    let (fg, keyword, string, comment, number) = (
        solid(t.fg),
        solid(t.ansi[5]),
        solid(t.ansi[2]),
        solid(t.dim_fg),
        solid(t.ansi[3]),
    );
    let preview = v_flex()
        .child(line([
            run("fn ", keyword),
            run("preview", solid(t.ansi[4])),
            run("() {", fg),
        ]))
        .child(line([
            run("    let ", keyword),
            run("id = ", fg),
            run("\"O0Il1\"", string),
            run(";", fg),
        ]))
        .child(line([
            run("    let ", keyword),
            run("n = ", fg),
            run("0x1F", number),
            run("; ", fg),
            run("// l1 I| O0", comment),
        ]))
        .child(line([run("}", fg)]));
    tile(&spec, "O0 Il1 {}", None, preview, solid(t.bg))
}

fn terminal_tile(spec: Spec) -> Div {
    let t = themes::term(|t| *t);
    let fg = solid(t.fg);
    let preview = v_flex()
        .child(line([
            run("~/studio ", solid(t.ansi[4])),
            run("main ", solid(t.ansi[5])),
            run("❯ ", ui::accent()),
            run("git diff --stat", fg),
        ]))
        .child(line([
            run(" fonts.rs   | 12 ", fg),
            run("++++++", solid(t.ansi[2])),
            run("--", solid(t.ansi[1])),
        ]))
        .child(line([
            run(" desk.rs    |  4 ", fg),
            run("++++", solid(t.ansi[2])),
        ]));
    // The theme cards' swatches, in the glyph band where they never clip.
    let swatches = h_flex().flex_none().gap_1().children(
        t.ansi[..8]
            .iter()
            .map(|&c| div().size_2().rounded_full().bg(solid(c))),
    );
    tile(&spec, "❯ ~/", Some(swatches), preview, solid(t.bg))
}
