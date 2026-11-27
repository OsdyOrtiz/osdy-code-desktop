//! The interface language: which one the user chose ([`Choice`]), which one
//! that means on this machine ([`Choice::resolve`]) and the Spanish text of
//! an English string ([`tr`]). English text is the key: code keeps writing
//! it, and a string with no translation yet stays in English.

mod es;

/// The key of the choice in the app's settings file.
pub const SETTINGS_KEY: &str = "language";

/// A language the interface is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English,
    Spanish,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::Spanish];

    /// Its id in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
        }
    }

    /// Its name in itself, as a language picker shows it.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
        }
    }

    /// The language of a locale tag (`es-AR`, `es_ES.UTF-8`, `en`):
    /// Spanish for any Spanish locale, English for everything else.
    pub fn from_locale(tag: &str) -> Language {
        let primary = tag
            .trim()
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default();
        if primary.eq_ignore_ascii_case("es") {
            Language::Spanish
        } else {
            Language::English
        }
    }
}

/// What the user picked: follow the operating system, or one language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Choice {
    #[default]
    System,
    Fixed(Language),
}

impl Choice {
    pub const ALL: [Choice; 3] = [
        Choice::System,
        Choice::Fixed(Language::English),
        Choice::Fixed(Language::Spanish),
    ];

    /// Its value in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Choice::System => "system",
            Choice::Fixed(language) => language.id(),
        }
    }

    /// The saved value; anything missing or unknown follows the system.
    pub fn from_id(id: Option<&str>) -> Choice {
        match id.map(str::trim) {
            Some("en") => Choice::Fixed(Language::English),
            Some("es") => Choice::Fixed(Language::Spanish),
            _ => Choice::System,
        }
    }

    /// The language shown: the fixed one, or the system locale's
    /// (English when the system's is unknown).
    pub fn resolve(self, system_locale: Option<&str>) -> Language {
        match self {
            Choice::Fixed(language) => language,
            Choice::System => system_locale.map_or(Language::English, Language::from_locale),
        }
    }
}

/// `en` in `language`: its translation, or `en` itself when there is none.
pub fn tr(language: Language, en: &'static str) -> &'static str {
    match language {
        Language::English => en,
        Language::Spanish => es::lookup(en).unwrap_or(en),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_locales_resolve_to_spanish_and_the_rest_to_english() {
        for tag in ["es", "es-AR", "es_ES.UTF-8", "ES-mx", " es-419 "] {
            assert_eq!(Language::from_locale(tag), Language::Spanish, "{tag}");
        }
        for tag in ["en-US", "pt-BR", "", "C", "est", "e"] {
            assert_eq!(Language::from_locale(tag), Language::English, "{tag}");
        }
    }

    #[test]
    fn the_system_choice_follows_the_locale_and_a_fixed_one_ignores_it() {
        assert_eq!(Choice::System.resolve(Some("es-AR")), Language::Spanish);
        assert_eq!(Choice::System.resolve(Some("en-US")), Language::English);
        assert_eq!(Choice::System.resolve(None), Language::English);
        let english = Choice::Fixed(Language::English);
        assert_eq!(english.resolve(Some("es-AR")), Language::English);
        let spanish = Choice::Fixed(Language::Spanish);
        assert_eq!(spanish.resolve(Some("en-US")), Language::Spanish);
    }

    #[test]
    fn a_saved_choice_round_trips_and_anything_else_follows_the_system() {
        for choice in Choice::ALL {
            assert_eq!(Choice::from_id(Some(choice.id())), choice);
        }
        assert_eq!(Choice::from_id(None), Choice::System);
        assert_eq!(Choice::from_id(Some("fr")), Choice::System);
        assert_eq!(Choice::from_id(Some("")), Choice::System);
    }

    #[test]
    fn spanish_translates_known_text_and_keeps_unknown_text_in_english() {
        assert_eq!(tr(Language::Spanish, "Language"), "Idioma");
        assert_eq!(tr(Language::English, "Language"), "Language");
        assert_eq!(tr(Language::Spanish, "No such text"), "No such text");
    }
}
