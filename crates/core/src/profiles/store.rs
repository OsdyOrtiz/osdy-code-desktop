//! The profiles store (gentle-pi `lib/agent-profiles.ts`): the
//! `profiles.json` format, profile names and the edits the screen makes.

use serde_json::{Map, Value, json};

use crate::i18n::{Language, tr};

use super::pretty;
use super::routing::{Routing, normalize_routing};

const PROFILES_KIND: &str = "gentle-pi.agent_model_profiles";

/// `isValidProfileName`: 1-64 of `[A-Za-z0-9._-]`, starting alphanumeric.
pub fn is_valid_profile_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !matches!(name, "constructor" | "prototype")
}

#[derive(Debug)]
pub struct ProfileError {
    key: &'static str,
    name: String,
}

impl ProfileError {
    fn missing(name: &str) -> Self {
        Self {
            key: "Profile does not exist: {name}.",
            name: name.to_string(),
        }
    }

    fn duplicate(name: &str) -> Self {
        Self {
            key: "Profile already exists: {name}.",
            name: name.to_string(),
        }
    }

    pub fn message(&self, language: Language) -> String {
        tr(language, self.key).replace("{name}", &self.name)
    }

    /// Retain the untranslated template and its display argument for live UI updates.
    pub fn into_parts(self) -> (&'static str, String) {
        (self.key, self.name)
    }
}

impl From<ProfileError> for String {
    fn from(error: ProfileError) -> Self {
        error.message(Language::English)
    }
}

fn require_valid_name(name: &str) -> Result<(), ProfileError> {
    if is_valid_profile_name(name) {
        Ok(())
    } else {
        Err(ProfileError {
            key: "Invalid profile name: {name}. Use 1-64 characters (letters, numbers, '.', '_', '-') starting with a letter or number.",
            name: format!("{name:?}"),
        })
    }
}

/// The orchestrator workflow a profile opts into (`OrchestratorMode` in
/// gentle-pi `lib/agent-profiles.ts`). It lives in the file's top-level
/// `workflow` map, keyed by profile, never inside the routing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrchestratorMode {
    /// Today's behaviour: the orchestrator may implement inline.
    #[default]
    Organic,
    /// The orchestrator never implements; every change goes to a bounded
    /// writer.
    FullDelegation,
}

impl OrchestratorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Organic => "organic",
            Self::FullDelegation => "full-delegation",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "organic" => Some(Self::Organic),
            "full-delegation" => Some(Self::FullDelegation),
            _ => None,
        }
    }
}

/// One profile's `workflow` entry (`ProfileWorkflow` in gentle-pi
/// `lib/agent-profiles.ts`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Workflow {
    mode: OrchestratorMode,
    /// Controlled Engineering: authoring rules against unrequested
    /// machinery, added to the prompt by Gentle Shell. Off by default and
    /// never written while off.
    controlled_engineering: bool,
}

impl Workflow {
    /// `isMeaningfulWorkflow`: an entry that differs from the defaults.
    fn is_meaningful(self) -> bool {
        self.mode != OrchestratorMode::Organic || self.controlled_engineering
    }

    /// `persistedWorkflow`: the mode always, the option only while on.
    fn to_json(self) -> Value {
        if self.controlled_engineering {
            json!({"orchestratorMode": self.mode.as_str(), "controlledEngineering": true})
        } else {
            json!({"orchestratorMode": self.mode.as_str()})
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfilesFile {
    pub active: Option<String>,
    pub profiles: Vec<(String, Routing)>,
    /// Profiles whose workflow is not the default; an all-default file has
    /// no `workflow` key at all.
    workflow: Vec<(String, Workflow)>,
    /// Top-level keys this version does not know, written back unchanged.
    extra: Map<String, Value>,
}

impl ProfilesFile {
    pub fn get(&self, name: &str) -> Option<&Routing> {
        self.profiles
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r)
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.profiles.iter().map(|(n, _)| n.as_str())
    }

    /// `readProfileWorkflow`: the profile's mode, the default when it has
    /// none (or does not exist).
    pub fn orchestrator_mode(&self, name: &str) -> OrchestratorMode {
        self.workflow_of(name).mode
    }

    /// Whether the profile turns Controlled Engineering on; off when it has
    /// no entry (or does not exist).
    pub fn controlled_engineering(&self, name: &str) -> bool {
        self.workflow_of(name).controlled_engineering
    }

    fn workflow_of(&self, name: &str) -> Workflow {
        self.workflow
            .iter()
            .find(|(n, _)| n == name)
            .map_or_else(Workflow::default, |(_, entry)| *entry)
    }

    /// Set `name`'s workflow in place; the defaults remove its entry.
    fn put_workflow(&mut self, name: &str, entry: Workflow) {
        match self.workflow.iter_mut().find(|(n, _)| n == name) {
            Some(slot) if entry.is_meaningful() => slot.1 = entry,
            Some(_) => self.workflow.retain(|(n, _)| n != name),
            None if entry.is_meaningful() => self.workflow.push((name.to_string(), entry)),
            None => {}
        }
    }
}

/// What normalization dropped while reading, so the screen can say so.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Drops {
    pub profiles: Vec<String>,
    pub agents: Vec<(String, String)>,
    pub active: Option<String>,
    /// `workflow` entries naming no surviving profile or no known mode, as
    /// (profile, mode as written).
    pub workflow: Vec<(String, String)>,
}

/// `normalizeProfilesFile`, keeping unknown top-level keys aside.
pub fn parse_profiles(text: &str) -> Option<(ProfilesFile, Drops)> {
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;
    if object.get("kind").and_then(Value::as_str) != Some(PROFILES_KIND)
        || object.get("version").and_then(Value::as_f64) != Some(1.0)
    {
        return None;
    }
    let raw = object.get("profiles")?.as_object()?;
    let mut drops = Drops::default();
    let mut profiles = Vec::new();
    for (name, raw_config) in raw {
        let config = is_valid_profile_name(name)
            .then(|| normalize_routing(raw_config))
            .flatten();
        let Some(config) = config else {
            drops.profiles.push(name.clone());
            continue;
        };
        if let Some(keys) = raw_config.as_object() {
            for agent in keys.keys().filter(|k| !config.contains(k)) {
                drops.agents.push((name.clone(), agent.clone()));
            }
        }
        profiles.push((name.clone(), config));
    }
    let active = match object.get("active") {
        None => None,
        Some(Value::String(name))
            if is_valid_profile_name(name) && profiles.iter().any(|(n, _)| n == name) =>
        {
            Some(name.clone())
        }
        Some(other) => {
            drops.active = Some(
                other
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| other.to_string()),
            );
            None
        }
    };
    let workflow = normalize_workflow(object.get("workflow"), &profiles, &mut drops);
    let extra = object
        .iter()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "kind" | "version" | "profiles" | "active" | "workflow"
            )
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Some((
        ProfilesFile {
            active,
            profiles,
            workflow,
            extra,
        },
        drops,
    ))
}

/// `normalizeWorkflowMap`: keeps opt-ins that name a surviving profile and a
/// known mode, reports the rest, and forgets all-default entries. Only exactly
/// `true` turns Controlled Engineering on; any other value reads as off and
/// drops nothing.
fn normalize_workflow(
    value: Option<&Value>,
    profiles: &[(String, Routing)],
    drops: &mut Drops,
) -> Vec<(String, Workflow)> {
    let Some(map) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut workflow = Vec::new();
    for (name, entry) in map {
        let raw = entry.get("orchestratorMode");
        let mode = raw
            .and_then(Value::as_str)
            .and_then(OrchestratorMode::parse);
        match mode {
            Some(mode) if profiles.iter().any(|(n, _)| n == name) => {
                let entry = Workflow {
                    mode,
                    controlled_engineering: entry.get("controlledEngineering")
                        == Some(&Value::Bool(true)),
                };
                if entry.is_meaningful() {
                    workflow.push((name.clone(), entry));
                }
            }
            _ => drops.workflow.push((
                name.clone(),
                match raw {
                    Some(Value::String(text)) => text.clone(),
                    Some(other) => other.to_string(),
                    None => entry.to_string(),
                },
            )),
        }
    }
    workflow
}

/// `serializeProfilesFile` (kind, version, profiles, active, workflow), then
/// any unknown top-level keys the file carried.
pub fn serialize_profiles(file: &ProfilesFile) -> String {
    let mut map = Map::new();
    map.insert("kind".into(), json!(PROFILES_KIND));
    map.insert("version".into(), json!(1));
    map.insert(
        "profiles".into(),
        Value::Object(
            file.profiles
                .iter()
                .map(|(n, r)| (n.clone(), r.to_json()))
                .collect(),
        ),
    );
    if let Some(active) = &file.active {
        map.insert("active".into(), json!(active));
    }
    if !file.workflow.is_empty() {
        map.insert(
            "workflow".into(),
            Value::Object(
                file.workflow
                    .iter()
                    .map(|(n, entry)| (n.clone(), entry.to_json()))
                    .collect(),
            ),
        );
    }
    for (k, v) in &file.extra {
        map.insert(k.clone(), v.clone());
    }
    pretty(&Value::Object(map))
}

pub fn create_profile(
    file: &ProfilesFile,
    name: &str,
    config: Routing,
) -> Result<ProfilesFile, String> {
    file.create(name, config).map_err(Into::into)
}

pub fn update_profile(
    file: &ProfilesFile,
    name: &str,
    config: Routing,
) -> Result<ProfilesFile, String> {
    file.update(name, config).map_err(Into::into)
}

pub fn duplicate_profile(
    file: &ProfilesFile,
    from: &str,
    to: &str,
) -> Result<ProfilesFile, String> {
    file.duplicate(from, to).map_err(Into::into)
}

pub fn rename_profile(file: &ProfilesFile, from: &str, to: &str) -> Result<ProfilesFile, String> {
    file.rename(from, to).map_err(Into::into)
}

pub fn delete_profile(file: &ProfilesFile, name: &str) -> Result<ProfilesFile, String> {
    file.delete(name).map_err(Into::into)
}

pub fn set_orchestrator_mode(
    file: &ProfilesFile,
    name: &str,
    mode: OrchestratorMode,
) -> Result<ProfilesFile, String> {
    file.with_orchestrator_mode(name, mode).map_err(Into::into)
}

pub fn set_controlled_engineering(
    file: &ProfilesFile,
    name: &str,
    enabled: bool,
) -> Result<ProfilesFile, String> {
    file.with_controlled_engineering(name, enabled)
        .map_err(Into::into)
}

pub fn set_active(file: &ProfilesFile, name: &str) -> Result<ProfilesFile, String> {
    file.with_active(name).map_err(Into::into)
}

impl ProfilesFile {
    pub fn create(&self, name: &str, config: Routing) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(name)?;
        if file.has(name) {
            return Err(ProfileError::duplicate(name));
        }
        let mut next = file.clone();
        next.profiles.push((name.to_string(), config));
        Ok(next)
    }

    pub fn update(&self, name: &str, config: Routing) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(name)?;
        let mut next = file.clone();
        let slot = next
            .profiles
            .iter_mut()
            .find(|(n, _)| n == name)
            .ok_or_else(|| ProfileError::missing(name))?;
        slot.1 = config;
        Ok(next)
    }

    /// `duplicateProfile`: the copy carries the source's workflow too.
    pub fn duplicate(&self, from: &str, to: &str) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(from)?;
        let config = file
            .get(from)
            .cloned()
            .ok_or_else(|| ProfileError::missing(from))?;
        let mut next = file.create(to, config)?;
        next.put_workflow(to, file.workflow_of(from));
        Ok(next)
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(from)?;
        if !file.has(from) {
            return Err(ProfileError::missing(from));
        }
        require_valid_name(to)?;
        if file.has(to) {
            return Err(ProfileError::duplicate(to));
        }
        let mut next = file.clone();
        // `renameProfile`: the workflow follows the profile to its new name.
        let names = next.profiles.iter_mut().map(|(n, _)| n);
        for name in names.chain(next.workflow.iter_mut().map(|(n, _)| n)) {
            if name == from {
                *name = to.to_string();
            }
        }
        if next.active.as_deref() == Some(from) {
            next.active = Some(to.to_string());
        }
        Ok(next)
    }

    pub fn delete(&self, name: &str) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(name)?;
        if !file.has(name) {
            return Err(ProfileError::missing(name));
        }
        if file.active.as_deref() == Some(name) {
            return Err(ProfileError {
                key: "Profile {name} is active. Apply another profile before deleting it.",
                name: name.to_string(),
            });
        }
        let mut next = file.clone();
        next.profiles.retain(|(n, _)| n != name);
        next.workflow.retain(|(n, _)| n != name);
        Ok(next)
    }

    /// `setProfileOrchestratorMode`: opt one existing profile into a mode; the
    /// default removes its entry.
    pub fn with_orchestrator_mode(
        &self,
        name: &str,
        mode: OrchestratorMode,
    ) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(name)?;
        if !file.has(name) {
            return Err(ProfileError::missing(name));
        }
        let mut next = file.clone();
        let entry = next.workflow_of(name);
        next.put_workflow(name, Workflow { mode, ..entry });
        Ok(next)
    }

    /// `setProfileControlledEngineering`: turn the option on or off for one
    /// existing profile, keeping its mode; off with the default mode removes its
    /// entry.
    pub fn with_controlled_engineering(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<Self, ProfileError> {
        let file = self;
        require_valid_name(name)?;
        if !file.has(name) {
            return Err(ProfileError::missing(name));
        }
        let mut next = file.clone();
        let entry = next.workflow_of(name);
        next.put_workflow(
            name,
            Workflow {
                controlled_engineering: enabled,
                ..entry
            },
        );
        Ok(next)
    }

    pub fn with_active(&self, name: &str) -> Result<Self, ProfileError> {
        require_valid_name(name)?;
        if !self.has(name) {
            return Err(ProfileError::missing(name));
        }
        let mut next = self.clone();
        next.active = Some(name.to_string());
        Ok(next)
    }

    pub fn export(name: &str, config: &Routing) -> Result<String, ProfileError> {
        require_valid_name(name)?;
        Ok(pretty(&json!({
            "kind": PROFILE_EXPORT_KIND,
            "version": 1,
            "name": name,
            "config": config.to_json(),
        })))
    }
}

/// `bootstrapProfilesFile`: a `current` profile from the routing in effect,
/// marked active only when it routes anything.
pub fn bootstrap_profiles(current: &Routing) -> ProfilesFile {
    let config = normalize_routing(&current.to_json()).unwrap_or_default();
    let active = (!config.0.is_empty()).then(|| "current".to_string());
    ProfilesFile {
        active,
        profiles: vec![("current".into(), config)],
        workflow: Vec::new(),
        extra: Map::new(),
    }
}

const PROFILE_EXPORT_KIND: &str = "gentle-pi.agent_model_profile";

/// The file `/gentle:profiles export` writes and `import` reads, in the
/// config home; the app's export suggests the same name.
pub const PROFILE_EXPORT_FILE: &str = "profiles.export.json";

/// One profile as exported (`serializeProfileExport`).
pub fn serialize_profile_export(name: &str, config: &Routing) -> Result<String, String> {
    ProfilesFile::export(name, config).map_err(Into::into)
}

/// A profile read from an export.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileExport {
    pub name: String,
    pub config: Routing,
    /// Agents whose entries were invalid and left out.
    pub dropped: Vec<String>,
}

/// `parseProfileExportTextWithDrops`: `None` unless it is a version 1
/// export with a valid name and a config object.
pub fn parse_profile_export(text: &str) -> Option<ProfileExport> {
    let value: Value = serde_json::from_str(text).ok()?;
    if value["kind"] != PROFILE_EXPORT_KIND || value["version"] != 1 {
        return None;
    }
    let name = value["name"]
        .as_str()
        .filter(|n| is_valid_profile_name(n))?;
    let config = normalize_routing(&value["config"])?;
    let dropped = value["config"]
        .as_object()?
        .keys()
        .filter(|agent| !config.contains(agent))
        .cloned()
        .collect();
    Some(ProfileExport {
        name: name.to_string(),
        config,
        dropped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::RouteEntry;

    #[test]
    fn profile_exports_round_trip_and_drop_invalid_entries() {
        let mut config = Routing::default();
        config.set(
            "orchestrator",
            entry(Some("openai/gpt-6-sol"), Some("high"), None),
        );
        config.set(
            "jd-judge-a",
            entry(Some("deepseek/deepseek-flash"), None, None),
        );
        let text = serialize_profile_export("work", &config).unwrap();
        assert!(text.contains("\"kind\": \"gentle-pi.agent_model_profile\""));
        let back = parse_profile_export(&text).unwrap();
        assert_eq!((back.name.as_str(), &back.config), ("work", &config));
        assert!(back.dropped.is_empty());

        // What Gentle Shell writes reads the same; bad agents are named.
        let shell = r#"{"kind":"gentle-pi.agent_model_profile","version":1,"name":"team",
            "config":{"orchestrator":{"model":"a/b"},"bad name!":{"model":"x/y"},"broken":{"thinking":"ultra"}}}"#;
        let parsed = parse_profile_export(shell).unwrap();
        assert_eq!(parsed.config.iter().count(), 1);
        assert_eq!(parsed.dropped, ["bad name!", "broken"]);

        assert!(serialize_profile_export("-bad", &config).is_err());
        // A profiles.json, another version, a bad name or no config is not an export.
        assert_eq!(
            parse_profile_export(r#"{"kind":"gentle-pi.agent_model_profiles","version":1}"#),
            None
        );
        assert_eq!(
            parse_profile_export(&shell.replace("\"version\":1", "\"version\":2")),
            None
        );
        assert_eq!(
            parse_profile_export(&shell.replace("\"team\"", "\"-x\"")),
            None
        );
        assert_eq!(
            parse_profile_export(
                r#"{"kind":"gentle-pi.agent_model_profile","version":1,"name":"a"}"#
            ),
            None
        );
        assert_eq!(parse_profile_export("not json"), None);
    }

    fn entry(model: Option<&str>, thinking: Option<&str>, tier: Option<&str>) -> RouteEntry {
        RouteEntry {
            model: model.map(str::to_string),
            thinking: thinking.map(str::to_string),
            tier: tier.map(str::to_string),
        }
    }

    #[test]
    fn parses_profiles_with_legacy_entries_and_drops() {
        let text = r#"{
          "kind": "gentle-pi.agent_model_profiles", "version": 1, "active": "work",
          "profiles": {
            "work": {
              "orchestrator": {"model": "openai/gpt-6-sol", "thinking": "high", "tier": "fast"},
              "gentle-ai-worker": "anthropic/claude-opus",
              "jd-judge-a": {"effort": "low"},
              "bad name!": {"model": "x/y"},
              "inherit-me": {},
              "broken": {"thinking": "ultra"}
            },
            "-bad": {}
          }
        }"#;
        let (file, drops) = parse_profiles(text).unwrap();
        assert_eq!(file.active.as_deref(), Some("work"));
        let work = file.get("work").unwrap();
        assert_eq!(
            work.orchestrator(),
            Some(&entry(Some("openai/gpt-6-sol"), Some("high"), Some("fast")))
        );
        assert_eq!(
            work.get("gentle-ai-worker"),
            Some(&entry(Some("anthropic/claude-opus"), None, None))
        );
        assert_eq!(
            work.get("jd-judge-a"),
            Some(&entry(None, Some("low"), None))
        );
        assert_eq!(work.get("inherit-me"), Some(&RouteEntry::default()));
        assert!(!work.contains("broken"));
        assert_eq!(drops.profiles, vec!["-bad".to_string()]);
        assert!(drops.agents.contains(&("work".into(), "bad name!".into())));
        assert!(drops.agents.contains(&("work".into(), "broken".into())));
        assert!(parse_profiles(r#"{"kind":"other","version":1,"profiles":{}}"#).is_none());
        assert!(
            parse_profiles(
                r#"{"kind":"gentle-pi.agent_model_profiles","version":2,"profiles":{}}"#
            )
            .is_none()
        );
        let (stale, drops) =
            parse_profiles(r#"{"kind":"gentle-pi.agent_model_profiles","version":1,"active":"gone","profiles":{}}"#).unwrap();
        assert_eq!(stale.active, None);
        assert_eq!(drops.active.as_deref(), Some("gone"));
    }

    #[test]
    fn validates_profile_names() {
        for good in ["a", "current", "Deep-work_2.1", &"x".repeat(64)] {
            assert!(is_valid_profile_name(good), "{good}");
        }
        for bad in [
            "",
            "-a",
            ".a",
            "_a",
            "a b",
            "a/b",
            "é",
            &"x".repeat(65),
            "constructor",
            "prototype",
            "__proto__",
        ] {
            assert!(!is_valid_profile_name(bad), "{bad}");
        }
        let file = bootstrap_profiles(&Routing::default());
        assert!(create_profile(&file, "bad name", Routing::default()).is_err());
        assert!(
            create_profile(&file, "current", Routing::default())
                .unwrap_err()
                .contains("already exists")
        );
        let renamed =
            rename_profile(&set_active(&file, "current").unwrap(), "current", "main").unwrap();
        assert_eq!(renamed.active.as_deref(), Some("main"));
        assert!(
            delete_profile(&renamed, "main")
                .unwrap_err()
                .contains("is active")
        );
    }

    const SHELL: &str = r#"{
      "kind": "gentle-pi.agent_model_profiles", "version": 1,
      "profiles": {"deep": {}, "fast": {}, "plain": {}},
      "workflow": {
        "deep": {"orchestratorMode": "full-delegation"},
        "plain": {"orchestratorMode": "organic"},
        "gone": {"orchestratorMode": "full-delegation"},
        "fast": {"orchestratorMode": "turbo"}
      }
    }"#;

    #[test]
    fn reads_gentle_shells_workflow_map_and_names_what_it_drops() {
        let (file, drops) = parse_profiles(SHELL).unwrap();
        assert_eq!(
            file.orchestrator_mode("deep"),
            OrchestratorMode::FullDelegation
        );
        assert_eq!(file.orchestrator_mode("plain"), OrchestratorMode::Organic);
        assert_eq!(file.orchestrator_mode("fast"), OrchestratorMode::Organic);
        assert_eq!(
            drops.workflow,
            [
                ("gone".to_string(), "full-delegation".to_string()),
                ("fast".to_string(), "turbo".to_string()),
            ]
        );
        // Only non-default modes of surviving profiles are written back.
        let text = serialize_profiles(&file);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value["workflow"],
            json!({"deep": {"orchestratorMode": "full-delegation"}})
        );
    }

    #[test]
    fn the_mode_follows_its_profile_through_every_edit() {
        let (file, _) = parse_profiles(SHELL).unwrap();
        let renamed = rename_profile(&file, "deep", "deeper").unwrap();
        assert_eq!(
            renamed.orchestrator_mode("deeper"),
            OrchestratorMode::FullDelegation
        );
        assert_eq!(renamed.orchestrator_mode("deep"), OrchestratorMode::Organic);

        let copied = duplicate_profile(&renamed, "deeper", "copy").unwrap();
        assert_eq!(
            copied.orchestrator_mode("copy"),
            OrchestratorMode::FullDelegation
        );

        let deleted = delete_profile(&copied, "deeper").unwrap();
        let cleared = set_orchestrator_mode(&deleted, "copy", OrchestratorMode::Organic).unwrap();
        let value: Value = serde_json::from_str(&serialize_profiles(&cleared)).unwrap();
        assert!(
            value.get("workflow").is_none(),
            "an all-organic file has no map"
        );

        assert!(set_orchestrator_mode(&file, "missing", OrchestratorMode::FullDelegation).is_err());
    }

    const CONTROLLED: &str = r#"{
      "kind": "gentle-pi.agent_model_profiles", "version": 1,
      "profiles": {"careful": {}, "both": {}, "loose": {}, "plain": {}},
      "workflow": {
        "careful": {"orchestratorMode": "organic", "controlledEngineering": true},
        "both": {"orchestratorMode": "full-delegation", "controlledEngineering": true},
        "loose": {"orchestratorMode": "full-delegation", "controlledEngineering": "yes"},
        "plain": {"orchestratorMode": "organic", "controlledEngineering": false}
      }
    }"#;

    #[test]
    fn reads_controlled_engineering_only_when_exactly_true() {
        let (file, drops) = parse_profiles(CONTROLLED).unwrap();
        assert!(file.controlled_engineering("careful"));
        assert!(file.controlled_engineering("both"));
        assert!(!file.controlled_engineering("loose"));
        assert!(!file.controlled_engineering("plain"));
        assert!(!file.controlled_engineering("missing"));
        // Another value drops nothing: the entry keeps its mode.
        assert_eq!(
            file.orchestrator_mode("loose"),
            OrchestratorMode::FullDelegation
        );
        assert!(drops.workflow.is_empty());
        // The mode is always written beside the option; off is never written.
        let value: Value = serde_json::from_str(&serialize_profiles(&file)).unwrap();
        assert_eq!(
            value["workflow"],
            json!({
                "careful": {"orchestratorMode": "organic", "controlledEngineering": true},
                "both": {"orchestratorMode": "full-delegation", "controlledEngineering": true},
                "loose": {"orchestratorMode": "full-delegation"}
            })
        );
    }

    #[test]
    fn controlled_engineering_follows_its_profile_through_every_edit() {
        let (file, _) = parse_profiles(CONTROLLED).unwrap();
        let renamed = rename_profile(&file, "careful", "strict").unwrap();
        assert!(renamed.controlled_engineering("strict"));
        assert!(!renamed.controlled_engineering("careful"));

        let copied = duplicate_profile(&renamed, "both", "copy").unwrap();
        assert!(copied.controlled_engineering("copy"));
        assert_eq!(
            copied.orchestrator_mode("copy"),
            OrchestratorMode::FullDelegation
        );

        // Changing the mode keeps the option, and the option keeps the mode.
        let organic = set_orchestrator_mode(&copied, "copy", OrchestratorMode::Organic).unwrap();
        assert!(organic.controlled_engineering("copy"));
        let on = set_controlled_engineering(&organic, "plain", true).unwrap();
        assert!(on.controlled_engineering("plain"));
        let off = set_controlled_engineering(&on, "both", false).unwrap();
        assert_eq!(
            off.orchestrator_mode("both"),
            OrchestratorMode::FullDelegation
        );

        let deleted = delete_profile(&off, "strict").unwrap();
        assert!(!deleted.controlled_engineering("strict"));

        let mut cleared = deleted;
        for name in ["both", "loose"] {
            cleared = set_orchestrator_mode(&cleared, name, OrchestratorMode::Organic).unwrap();
        }
        for name in ["copy", "plain"] {
            cleared = set_controlled_engineering(&cleared, name, false).unwrap();
        }
        let value: Value = serde_json::from_str(&serialize_profiles(&cleared)).unwrap();
        assert!(
            value.get("workflow").is_none(),
            "an all-default file has no map"
        );

        assert!(set_controlled_engineering(&file, "missing", true).is_err());
    }
}
