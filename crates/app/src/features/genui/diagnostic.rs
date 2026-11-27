//! Display only: the core validators also supply errors to non-UI callers.
//! Keep their English diagnostics intact there, and keep spec values and IDs
//! verbatim when interpolating translated diagnostics here.

use gentle_core::i18n::{Language, tr};

use crate::shared::i18n::{Message, language};

pub(super) fn render_all(errors: &[String], separator: &str) -> String {
    errors
        .iter()
        .map(|error| render(error))
        .collect::<Vec<_>>()
        .join(separator)
}

pub(super) fn render(error: &str) -> String {
    let language = language();
    if language == Language::English {
        return error.to_owned();
    }
    let translated = tr(language, error);
    if translated != error {
        return translated.to_owned();
    }
    // Match whole diagnostics, not substrings of values or identifiers.
    for (prefix, between, suffix, template) in [
        (
            "edge \"",
            "\": node \"",
            "\" does not exist",
            "edge \"{first}\": node \"{second}\" does not exist",
        ),
        (
            "node \"",
            "\": group \"",
            "\" is not in groups",
            "node \"{first}\": group \"{second}\" is not in groups",
        ),
    ] {
        if let Some((first, second)) = error
            .strip_prefix(prefix)
            .and_then(|line| line.strip_suffix(suffix))
            .and_then(|line| line.split_once(between))
        {
            return Message::with(
                template,
                [
                    ("{first}", first.to_string()),
                    ("{second}", second.to_string()),
                ],
            )
            .render_in(language);
        }
    }
    for (prefix, suffix, template) in [
        ("unknown kind \"", "\"", "unknown kind \"{detail}\""),
        ("unknown column \"", "\"", "unknown column \"{detail}\""),
        ("unknown row \"", "\"", "unknown row \"{detail}\""),
        ("unknown node \"", "\"", "unknown node \"{detail}\""),
        ("unknown edge \"", "\"", "unknown edge \"{detail}\""),
        ("unknown group \"", "\"", "unknown group \"{detail}\""),
        (
            "node \"",
            "\" already exists",
            "node \"{detail}\" already exists",
        ),
        (
            "edge \"",
            "\" already exists",
            "edge \"{detail}\" already exists",
        ),
        (
            "group \"",
            "\" already exists",
            "group \"{detail}\" already exists",
        ),
        (
            "too many nodes (at most ",
            ")",
            "too many nodes (at most {detail})",
        ),
        (
            "too many edges (at most ",
            ")",
            "too many edges (at most {detail})",
        ),
        (
            "too many groups (at most ",
            ")",
            "too many groups (at most {detail})",
        ),
    ] {
        if let Some(detail) = error
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix(suffix))
        {
            return Message::with(template, [("{detail}", detail.to_string())]).render_in(language);
        }
    }
    error.to_owned()
}
