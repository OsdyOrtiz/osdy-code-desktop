//! This app's version, license and links, the people behind it (the
//! repository's `CREDITS.md`, built in) and where the third-party licenses
//! are.

use std::sync::LazyLock;

use gentle_core::credits::{self, Group};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::super::{
    app,
    layout::{row, section},
};
use crate::shared::i18n::t;
use crate::shared::kisses::{HEART, paint_sprite};
use crate::shared::ui;

const REPOSITORY: &str = "https://github.com/OsdyOrtiz/osdy-code-desktop";
const ISSUES: &str = "https://github.com/OsdyOrtiz/osdy-code-desktop/issues";
/// The README's license notice: MIT, and the trademark terms that go with it.
const LICENSE: &str = "https://github.com/OsdyOrtiz/osdy-code-desktop#license";
/// The animated scenes' license, the one third-party license file in the
/// repository.
const SCENES_LICENSE: &str =
    "https://github.com/OsdyOrtiz/osdy-code-desktop/blob/main/crates/app/themes/scenes/LICENSE";
/// Every crate built into the app, each under the license crates.io lists.
const DEPENDENCIES: &str = "https://github.com/OsdyOrtiz/osdy-code-desktop/blob/main/Cargo.lock";
/// The author of the Osdy Code version and their site.
const AUTHOR: &str = "Osdy Ortiz";
const AUTHOR_ROLE: &str = "Full-Stack Developer · AI Software Engineer";
const AUTHOR_MOTTO: &str = "Conocimiento de Senior, Corazón de Junior";
const WEBSITE: &str = "https://landing-osdy.vercel.app/";

static CREDITS: LazyLock<Vec<Group>> =
    LazyLock::new(|| credits::parse(include_str!("../../../../../../CREDITS.md")));

pub(super) fn render() -> Div {
    v_flex()
        .gap_3()
        .child(about_section())
        .child(credits_section())
        .child(third_party_section())
}

fn link(id: &'static str, label: &'static str, url: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .icon(Icon::new(IconName::ExternalLink))
        .label(label)
        .on_click(move |_, _, cx| cx.open_url(url))
}

fn about_section() -> Div {
    section(IconName::Info, "Osdy Code Desktop")
        .child(row(t("Version"), app::version()))
        .child(row(t("License"), "MIT"))
        .child(row(t("Author"), AUTHOR))
        .child(row(t("Fork of"), "Gentleman-Programming/gentle-shell-gpui"))
        .child(author_card())
        .child(
            h_flex()
                .pt_2()
                .gap_2()
                .flex_wrap()
                .child(link("about-repository", t("Repository"), REPOSITORY))
                .child(link("about-issues", t("Issues"), ISSUES))
                .child(link("about-license", t("License"), LICENSE))
                .child(link("about-website", t("Website"), WEBSITE)),
        )
}

/// The author's role and motto, with the pixel heart from the welcome finale.
fn author_card() -> Div {
    const DOT: f32 = 2.;
    v_flex()
        .pt_2()
        .gap_1()
        .child(div().text_sm().text_color(ui::fg()).child(AUTHOR_ROLE))
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(div().text_xs().text_color(ui::muted()).child(AUTHOR_MOTTO))
                .child(
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
                        ),
                ),
        )
}

/// One labelled group per heading in `CREDITS.md`; a name links to the
/// person's GitHub profile when the file names their account.
fn credits_section() -> Div {
    section(IconName::Heart, t("Credits")).children(CREDITS.iter().enumerate().map(
        |(ix, group)| {
            v_flex()
                .id(("about-credits", ix))
                .pt_2()
                .gap_1()
                .child(ui::section_label(match group.title.as_str() {
                    "Contributors" => t("Contributors").to_string(),
                    _ => group.title.clone(),
                }))
                .child(
                    h_flex()
                        .gap_1()
                        .flex_wrap()
                        .children(group.people.iter().enumerate().map(|(ix, person)| {
                            match person.login.as_ref().zip(person.profile_url()) {
                                Some((login, url)) => Button::new(("about-credit", ix))
                                    .ghost()
                                    .small()
                                    .label(person.name.clone())
                                    .tooltip(t("@{login} on GitHub").replace("{login}", login))
                                    .on_click(move |_, _, cx| cx.open_url(&url))
                                    .into_any_element(),
                                None => div()
                                    .px_2()
                                    .text_sm()
                                    .text_color(ui::fg())
                                    .child(person.name.clone())
                                    .into_any_element(),
                            }
                        })),
                )
        },
    ))
}

fn third_party_section() -> Div {
    section(IconName::FileText, t("Third-party licenses"))
        .child(div().text_xs().text_color(ui::muted()).child(t(
            "The animated scenes and the Rust crates built into the app keep their own licenses.",
        )))
        .child(
            h_flex()
                .pt_2()
                .gap_2()
                .flex_wrap()
                .child(link(
                    "about-scenes-license",
                    t("Scenes license"),
                    SCENES_LICENSE,
                ))
                .child(link("about-dependencies", t("Dependencies"), DEPENDENCIES)),
        )
}
