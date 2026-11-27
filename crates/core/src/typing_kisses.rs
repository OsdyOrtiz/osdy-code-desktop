//! Whether the composer's paws and notes play, saved as `composerParticles`
//! in the app's `settings.json`.

use serde_json::Value;

/// The `composerParticles` key of the app's settings file.
pub const KEY: &str = "composerParticles";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypingKisses {
    /// They play, except while the system asks to reduce motion.
    #[default]
    Auto,
    /// Never.
    Off,
}

impl TypingKisses {
    pub const ALL: [TypingKisses; 2] = [TypingKisses::Auto, TypingKisses::Off];

    pub fn label(self) -> &'static str {
        match self {
            TypingKisses::Auto => "Auto",
            TypingKisses::Off => "Off",
        }
    }

    /// One line for Configuration, next to the choice: what it does.
    pub fn summary(self) -> &'static str {
        match self {
            TypingKisses::Auto => {
                "Paws and notes float up as you type, unless the system reduces motion."
            }
            TypingKisses::Off => "No paws or notes while typing.",
        }
    }

    /// The saved value: `"off"` is Off; anything else, or nothing, is Auto.
    pub fn from_json(value: Option<&Value>) -> Self {
        match value.and_then(Value::as_str) {
            Some(text) if text.eq_ignore_ascii_case("off") => TypingKisses::Off,
            _ => TypingKisses::Auto,
        }
    }

    pub fn to_json(self) -> Value {
        Value::String(self.label().to_ascii_lowercase())
    }

    /// Whether typing particles play, given the system's reduce-motion setting.
    pub fn plays(self, reduce_motion: bool) -> bool {
        self == TypingKisses::Auto && !reduce_motion
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_off_turns_them_off_and_it_round_trips() {
        assert_eq!(TypingKisses::from_json(None), TypingKisses::Auto);
        assert_eq!(
            TypingKisses::from_json(Some(&json!("OFF"))),
            TypingKisses::Off
        );
        // Unknown or mistyped values keep the default rather than silencing.
        assert_eq!(
            TypingKisses::from_json(Some(&json!(false))),
            TypingKisses::Auto
        );
        assert_eq!(
            TypingKisses::from_json(Some(&json!("none"))),
            TypingKisses::Auto
        );
        for choice in TypingKisses::ALL {
            assert_eq!(TypingKisses::from_json(Some(&choice.to_json())), choice);
        }
    }

    #[test]
    fn auto_follows_reduce_motion_and_off_never_plays() {
        assert!(TypingKisses::Auto.plays(false));
        assert!(!TypingKisses::Auto.plays(true));
        assert!(!TypingKisses::Off.plays(false));
    }
}
