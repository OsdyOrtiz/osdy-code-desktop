//! The pre-release pose for the embedded Mapache Neon Tokyo scene on Welcome.

use std::sync::{Arc, LazyLock};

use crate::shared::themes::{Source, Theme};

const SHIPPED_NAME: &str = "Mapache Neon Tokyo";
const THEME_JSON: &str = include_str!("../../../themes/examples/mapache-neon-tokyo.json");

static SCENE_CODE: LazyLock<String> = LazyLock::new(|| {
    let theme: serde_json::Value =
        serde_json::from_str(THEME_JSON).expect("valid embedded Mapache Neon Tokyo theme");
    theme["background"]["sceneCode"]
        .as_str()
        .expect("embedded Mapache Neon Tokyo scene source")
        .to_owned()
});

// Engine loads this in the same JS block as the theme, so `first` and
// `RELEASE` are the original scene's timeline origin and rose-drop cue.
const START_BEFORE_RELEASE: &str = r#"
{
  const scene = SCENES['mapache-neon-tokyo'];
  const draw = scene.draw;
  scene.draw = function(g, t) {
    if (first === null) first = t - (RELEASE - 0.125);
    return draw.call(this, g, t);
  };
}
"#;

static WELCOME_CODE: LazyLock<Arc<str>> =
    LazyLock::new(|| format!("{}{START_BEFORE_RELEASE}", SCENE_CODE.as_str()).into());

/// Override only the built-in catalog entry with its original scene code.
pub(super) fn rose_scene(theme: &Theme) -> Option<Arc<str>> {
    // Shipped examples and imports are both Source::App; imports have a file.
    if theme.source != Source::App || theme.file.is_some() || theme.name != SHIPPED_NAME {
        return None;
    }
    let scene = theme.background.as_ref()?.scene.as_ref()?;
    (scene.code.as_ref() == SCENE_CODE.as_str()).then(|| WELCOME_CODE.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::backdrop::scene::Engine;
    use crate::shared::themes::{example_themes, parse_pi_theme};
    use std::path::Path;

    fn shipped() -> Theme {
        example_themes()
            .into_iter()
            .find(|theme| theme.name == SHIPPED_NAME)
            .unwrap()
    }

    #[test]
    fn welcome_rose_only_overrides_the_exact_scene_with_stable_source() {
        let mut altered = shipped();
        let backdrop = altered.background.as_mut().unwrap();
        let scene = backdrop.scene.as_mut().unwrap();
        scene.code = format!("{}\n// custom scene", SCENE_CODE.as_str()).into();
        assert!(rose_scene(&altered).is_none());
        let first = rose_scene(&shipped()).unwrap();
        let second = rose_scene(&shipped()).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn welcome_rose_ignores_copies_of_the_shipped_theme() {
        let mut json: serde_json::Value = serde_json::from_str(THEME_JSON).unwrap();
        let copied = parse_pi_theme(THEME_JSON, Source::App, Some(Path::new("copy.json"))).unwrap();
        json["name"] = serde_json::json!("Mapache Neon Tokyo Copy");
        let renamed = parse_pi_theme(&json.to_string(), Source::App, None).unwrap();
        assert!(rose_scene(&renamed).is_none(), "renamed catalog entry");
        assert!(rose_scene(&copied).is_none(), "same-name imported file");
    }

    #[test]
    fn welcome_rose_holds_before_release_and_resumes_past_it() {
        let code = SCENE_CODE.as_str();
        let wrapped_code = rose_scene(&shipped()).unwrap();
        let original = Engine::new(code, "{}", 22.).unwrap();
        let welcome = Engine::new(&wrapped_code, "{}", 22.).unwrap();
        let frame = |engine: &Engine, time| {
            let (commands, points) = engine.frame((800., 600.), time, None).unwrap();
            // Path terminators are NaNs; compare their encoded bits, too.
            let bits = |values: Vec<f64>| values.into_iter().map(f64::to_bits).collect::<Vec<_>>();
            (bits(commands), bits(points))
        };
        frame(&original, 0.);

        let held = frame(&welcome, 20.);
        let resumed = frame(&welcome, 20.25);
        assert!(
            held == frame(&original, 9.125),
            "expected the pre-release pose"
        );
        assert!(
            resumed == frame(&original, 9.375),
            "expected the pose after release"
        );
        assert!(held != resumed, "resuming should advance the scene");
    }
}
