//! Spanish text of the interface, by its English text, one table per area.
//! A duplicated key within a table is an unreachable pattern, which the
//! build's lints reject; the first table that knows a text wins.

mod agent_session;
mod files_git;
mod genui;
mod profiles_usage;
mod settings_a;
mod settings_b;
mod setup_signin;
mod shell_tools;
mod workspace_tabs;

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    common(en)
        .or_else(|| settings_a::lookup(en))
        .or_else(|| settings_b::lookup(en))
        .or_else(|| workspace_tabs::lookup(en))
        .or_else(|| agent_session::lookup(en))
        .or_else(|| shell_tools::lookup(en))
        .or_else(|| genui::lookup(en))
        .or_else(|| files_git::lookup(en))
        .or_else(|| profiles_usage::lookup(en))
        .or_else(|| setup_signin::lookup(en))
}

fn common(en: &str) -> Option<&'static str> {
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
        // Session start screen.
        "Sign in to start cooking" => "Inicia sesión para empezar",
        "No models are available yet. Sign in with a subscription or add an API key." => {
            "Todavía no hay modelos disponibles. Inicia sesión con una suscripción o agrega una API key."
        }
        "Sign in to a provider" => "Iniciar sesión en un proveedor",
        _ => return None,
    })
}
