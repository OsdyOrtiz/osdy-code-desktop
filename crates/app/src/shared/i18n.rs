//! The interface language (Settings › Appearance › Language): the user's
//! [`Choice`], saved in the app's `settings.json` (`language`), and the
//! language it resolves to, kept process-wide so any render helper can
//! translate with [`t`] without a context. The resolved ID is also shared
//! with the osdy-gentle extension through the agent locale file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use gentle_core::i18n::{self, Choice, Language};
use gpui_kit::component::input::InputState;
use gpui_kit::{App, Entity, Global, Window};

use super::themes;

pub const AGENT_LOCALE_ENV: &str = "OSDY_DESKTOP_LOCALE_FILE";

pub fn agent_locale_path() -> Option<PathBuf> {
    std::path::absolute(super::data_dir::app()?.join("agent-locale")).ok()
}

fn write_agent_locale(path: &Path, language: Language) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, i18n::agent_locale_contents(language))
}

/// The resolved language, as an index into [`Language::ALL`].
static CURRENT: AtomicU8 = AtomicU8::new(0);

struct UiLanguage(Choice);

impl Global for UiLanguage {}

/// Read the saved choice and resolve it against the system's locale.
pub fn init(cx: &mut App) {
    let saved = themes::store_path()
        .and_then(|path| themes::read_key(&path, i18n::SETTINGS_KEY))
        .and_then(|value| value.as_str().map(str::to_string));
    let choice = Choice::from_id(saved.as_deref());
    resolve(choice);
    cx.set_global(UiLanguage(choice));
}

fn resolve(choice: Choice) {
    let language = choice.resolve(sys_locale::get_locale().as_deref());
    let index = Language::ALL
        .iter()
        .position(|l| *l == language)
        .unwrap_or(0);
    CURRENT.store(index as u8, Ordering::Relaxed);
    if let Some(path) = agent_locale_path()
        && let Err(err) = write_agent_locale(&path, language)
    {
        eprintln!("gentle-shell-gpui: could not share the agent locale: {err}");
    }
}

#[cfg(test)]
thread_local! {
    static TEST_LANGUAGE: std::cell::Cell<Option<Language>> = const { std::cell::Cell::new(None) };
}

/// Exercise a live redraw or reply without saving settings or changing other tests' language.
#[cfg(test)]
pub(crate) fn with_test_language<R>(language: Language, test: impl FnOnce() -> R) -> R {
    struct Reset(Option<Language>);
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_LANGUAGE.set(self.0);
        }
    }
    let _reset = Reset(TEST_LANGUAGE.replace(Some(language)));
    test()
}

#[cfg(test)]
#[test]
fn agent_locale_is_written_at_the_shared_path_and_updated() {
    let dir = std::env::temp_dir().join(format!("osdy-agent-locale-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Scratch(dir);
    let path = root.0.join("app").join("agent-locale");
    write_agent_locale(&path, Language::Spanish).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "es");
    write_agent_locale(&path, Language::English).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "en");
}

/// The language the interface is shown in.
pub fn language() -> Language {
    #[cfg(test)]
    if let Some(language) = TEST_LANGUAGE.get() {
        return language;
    }
    Language::ALL
        .get(CURRENT.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or_default()
}

/// The user's choice.
pub fn choice(cx: &App) -> Choice {
    cx.try_global::<UiLanguage>()
        .map_or_else(Choice::default, |c| c.0)
}

/// Choose and save the language; every window redraws in it. Written at
/// once (a click, a small file), like the other single-key settings.
pub fn set_choice(choice: Choice, cx: &mut App) {
    let previous = language();
    cx.set_global(UiLanguage(choice));
    resolve(choice);
    if previous != language() {
        crate::app::set_menus(cx);
    }
    cx.refresh_windows();
    let Some(path) = themes::store_path() else {
        return;
    };
    let value = serde_json::Value::String(choice.id().to_string());
    if let Err(err) = themes::write_key(&path, i18n::SETTINGS_KEY, value) {
        eprintln!("gentle-shell-gpui: could not save the language: {err}");
    }
}

/// `en` in the interface language (itself when it has no translation).
pub fn t(en: &'static str) -> &'static str {
    i18n::tr(language(), en)
}

/// A retained message: translate its template when displayed, not when stored.
/// Details from another process or the OS stay verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    template: &'static str,
    details: Vec<(&'static str, String)>,
}

impl Message {
    pub fn new(template: &'static str) -> Self {
        Self {
            template,
            details: Vec::new(),
        }
    }

    pub fn with(
        template: &'static str,
        details: impl IntoIterator<Item = (&'static str, String)>,
    ) -> Self {
        Self {
            template,
            details: details.into_iter().collect(),
        }
    }

    pub fn with_error(template: &'static str, error: impl std::fmt::Display) -> Self {
        Self::with(template, [("{error}", error.to_string())])
    }

    pub fn raw(detail: impl Into<String>) -> Self {
        Self::with("{detail}", [("{detail}", detail.into())])
    }

    pub fn render(&self) -> String {
        self.render_in(language())
    }

    pub fn render_in(&self, language: Language) -> String {
        let mut remaining = i18n::tr(language, self.template);
        let mut rendered = String::new();
        // Search only the template, never the inserted details.
        while let Some((index, key, value)) = self
            .details
            .iter()
            .filter_map(|(key, value)| remaining.find(*key).map(|index| (index, key, value)))
            .min_by_key(|(index, _, _)| *index)
        {
            rendered.push_str(&remaining[..index]);
            rendered.push_str(value);
            remaining = &remaining[index + key.len()..];
        }
        rendered.push_str(remaining);
        rendered
    }
}

/// Text kept outside render (an input's placeholder) follows the language:
/// a view keeps the `seen` language it last wrote it in and calls this from
/// `render`; it writes `en` again only after a change, so a render never
/// triggers another.
pub fn sync_placeholder(
    input: &Entity<InputState>,
    en: &'static str,
    seen: &mut Language,
    window: &mut Window,
    cx: &mut App,
) {
    let now = language();
    if *seen != now {
        *seen = now;
        input.update(cx, |state, cx| state.set_placeholder(t(en), window, cx));
    }
}
