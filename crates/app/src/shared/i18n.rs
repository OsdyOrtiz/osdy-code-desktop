//! The interface language (Settings › Appearance › Language): the user's
//! [`Choice`], saved in the app's `settings.json` (`language`), and the
//! language it resolves to, kept process-wide so any render helper can
//! translate with [`t`] without a context. It only changes the app's own
//! text: nothing of it reaches the agent.

use std::sync::atomic::{AtomicU8, Ordering};

use gentle_core::i18n::{self, Choice, Language};
use gpui_kit::{App, Global};

use super::themes;

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
}

/// The language the interface is shown in.
pub fn language() -> Language {
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
    cx.set_global(UiLanguage(choice));
    resolve(choice);
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
