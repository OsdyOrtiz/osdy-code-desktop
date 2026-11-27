//! Spanish text of the interface, by its English text. A duplicated key is
//! an unreachable pattern, which the build's lints reject.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        // Language section.
        "Language" => "Idioma",
        "System" => "Sistema",
        "Interface language" => "Idioma de la interfaz",
        "Follows the operating system's language: Spanish when it is set to Spanish, English otherwise." => {
            "Sigue el idioma del sistema operativo: español si está en español, inglés en los demás casos."
        }
        "Only the app's interface changes; the agent keeps answering in the language you write to it. Saved in the app's settings.json (language)." => {
            "Solo cambia la interfaz de la app; el agente sigue respondiendo en el idioma en que le escribes. Se guarda en el settings.json de la app (language)."
        }
        _ => return None,
    })
}
