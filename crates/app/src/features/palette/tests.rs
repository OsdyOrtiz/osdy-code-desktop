use gpui_kit::{AppContext as _, TestAppContext};

use super::{Category, Language, PaletteAction, PaletteInput, PaletteView, TabLayout};
use crate::features::session::SessionView;
use crate::shared::i18n::with_test_language;

#[gpui::test]
fn t14_palette_finds_copy_by_english_and_spanish_description(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let root = std::env::temp_dir().join(format!("ocd-palette-search-{}", std::process::id()));
    for (language, queries, expected_detail) in [
        (
            Language::English,
            &["clipboard"][..],
            "Copy the last answer to the clipboard",
        ),
        (
            Language::Spanish,
            &["portapapeles", "clipboard"][..],
            "Copiar la última respuesta al portapapeles",
        ),
    ] {
        with_test_language(language, || {
            let (palette, cx) = cx.add_window_view(|window, cx| {
                let session = cx.new(|cx| SessionView::new(root.clone(), None, window, cx, false));
                let input = PaletteInput {
                    commands: session.read(cx).commands(),
                    workspaces: Vec::new(),
                    root: root.clone(),
                    category: Category::Commands,
                    tab_layout: TabLayout::Horizontal,
                    browser_card: None,
                };
                PaletteView::new(input, window, cx)
            });
            for query in queries {
                palette.update_in(cx, |palette, window, cx| {
                    palette
                        .query
                        .update(cx, |state, cx| state.set_value(*query, window, cx));
                    palette.refresh_rows(cx);
                    assert!(
                        palette.rows.iter().any(|row| {
                            row.title == "/copy"
                                && row.detail == expected_detail
                                && row.action == PaletteAction::Command("copy".into())
                        }),
                        "{language:?}: {query} should find /copy with {expected_detail:?}"
                    );
                });
            }
        });
    }
}
