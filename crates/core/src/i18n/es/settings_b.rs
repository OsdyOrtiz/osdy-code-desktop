//! Spanish text of Settings (agents, models, devices, integrations, notifications, extensions).

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        "Agents" => "Agentes",
        _ => return None,
    })
}
