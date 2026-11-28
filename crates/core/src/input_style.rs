//! Input style: how an agent tab's composer looks, the person's choice in
//! Configuration › Appearance, saved in the app's `settings.json`
//! ([`SETTINGS_KEY`]) and followed by every tab.

use serde_json::Value;

/// Key of the app's `settings.json`.
pub const SETTINGS_KEY: &str = "inputStyle";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputStyle {
    /// A floating card, its controls inside; folder and branch below.
    Card,
    /// Docked across the pane on a hairline; one row of flat chips under
    /// the text, a ↵ keycap or "Stop esc" at its end.
    Bar,
    /// A framed `❯` line in the code font over a terminal status strip
    /// along the pane's bottom edge.
    Prompt,
    /// One rounded pill: attach, text and send on the same line.
    Pill,
    /// A soft rounded box: the text on top, quiet round controls and a
    /// filled send button in a row below it. The default.
    #[default]
    Soft,
}

impl InputStyle {
    pub const ALL: [InputStyle; 5] = [
        InputStyle::Card,
        InputStyle::Bar,
        InputStyle::Prompt,
        InputStyle::Pill,
        InputStyle::Soft,
    ];

    /// What the settings file calls it.
    pub fn id(self) -> &'static str {
        match self {
            InputStyle::Card => "card",
            InputStyle::Bar => "bar",
            InputStyle::Prompt => "prompt",
            InputStyle::Pill => "pill",
            InputStyle::Soft => "soft",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            InputStyle::Card => "Card",
            InputStyle::Bar => "Bar",
            InputStyle::Prompt => "Prompt",
            InputStyle::Pill => "Pill",
            InputStyle::Soft => "Soft",
        }
    }

    /// One line for Configuration, under the choice.
    pub fn summary(self) -> &'static str {
        match self {
            InputStyle::Card => {
                "A floating card with its controls inside; folder and branch below."
            }
            InputStyle::Bar => "Docked across the pane: one row of chips, a ↵ key and Stop · esc.",
            InputStyle::Prompt => "A framed ❯ line in the code font over a terminal status strip.",
            InputStyle::Pill => "One rounded pill: attach, text and send on the same line.",
            InputStyle::Soft => {
                "A soft rounded box: the text on top, quiet controls and a filled send."
            }
        }
    }

    /// The saved value; anything unreadable (or from a newer build) is the
    /// default.
    pub fn from_json(value: &Value) -> InputStyle {
        value
            .as_str()
            .and_then(|text| {
                Self::ALL
                    .into_iter()
                    .find(|style| style.id().eq_ignore_ascii_case(text.trim()))
            })
            .unwrap_or_default()
    }

    pub fn to_json(self) -> Value {
        Value::String(self.id().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_styles_round_trip_and_unknown_ones_read_as_soft() {
        for style in InputStyle::ALL {
            assert_eq!(InputStyle::from_json(&style.to_json()), style);
        }
        assert_eq!(
            InputStyle::from_json(&Value::from(" Pill ")),
            InputStyle::Pill
        );
        assert_eq!(
            InputStyle::from_json(&Value::from("neon")),
            InputStyle::Soft
        );
        assert_eq!(InputStyle::from_json(&Value::Null), InputStyle::Soft);
        assert_eq!(InputStyle::default(), InputStyle::Soft);
    }
}
