//! Spanish text of Settings (rail, pages, appearance, general, updates, about).

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        "Appearance" => "Apariencia",
        _ => return None,
    })
}
