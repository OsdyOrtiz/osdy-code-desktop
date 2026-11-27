//! Reading the checks: readiness, the install plan, row tone and the
//! install log's tail. No GPUI here.

use std::collections::VecDeque;

use crate::features::setup::{Check, CheckId, Detail, Fix, Need, Status};
use crate::shared::i18n::language;

/// Install output kept per run (the failure view shows the tail).
const LOG_KEEP: usize = 200;

pub(super) fn is_ok(check: &Check) -> bool {
    matches!(check.status, Status::Ok(_))
}

/// A Required check that holds up Continue. Signing in to a provider is
/// required to chat but can wait: Providers and the chat's own "Sign in"
/// take care of it later.
pub(super) fn blocks(check: &Check) -> bool {
    check.need == Need::Required && check.fix != Fix::SignIn
}

/// Every Required check that holds up Continue passes.
pub(super) fn required_ok(checks: &[Check]) -> bool {
    checks.iter().filter(|c| blocks(c)).all(is_ok)
}

/// Everything but the optional extras passes.
pub(super) fn all_set(checks: &[Check]) -> bool {
    checks
        .iter()
        .filter(|c| c.need != Need::Optional)
        .all(is_ok)
}

/// gentle-shell can start: the launcher and what it runs on are in place
/// (setup and sign-in can still be pending; the launcher handles those).
pub(super) fn usable(checks: &[Check]) -> bool {
    checks
        .iter()
        .filter(|c| matches!(c.id, CheckId::Node | CheckId::Go | CheckId::GentleShell))
        .all(is_ok)
}

/// What "Install everything" runs, in screen order: every Required or
/// Recommended check an unattended install fixes that isn't done.
pub(super) fn install_plan(checks: &[Check]) -> VecDeque<CheckId> {
    checks
        .iter()
        .filter(|c| c.need != Need::Optional && c.fix == Fix::Auto && !is_ok(c))
        .map(|c| c.id)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Good,
    Warn,
    Bad,
}

/// Green when done; red for a missing requirement or a failure; amber for
/// anything outdated or missing that can wait.
pub(super) fn tone(check: &Check) -> Tone {
    match &check.status {
        Status::Ok(_) => Tone::Good,
        Status::Failed(_) => Tone::Bad,
        Status::Missing(_) if blocks(check) => Tone::Bad,
        Status::Missing(_) | Status::Outdated(_) => Tone::Warn,
    }
}

pub(super) fn status_text(status: &Status) -> String {
    match status {
        Status::Ok(s) | Status::Missing(s) | Status::Outdated(s) | Status::Failed(s) => {
            s.text(language())
        }
    }
}

/// Append install output, one entry per non-empty line, keeping the tail.
pub(super) fn push_lines(log: &mut Vec<String>, chunk: &str) {
    log.extend(
        chunk
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string),
    );
    if log.len() > LOG_KEEP {
        log.drain(..log.len() - LOG_KEEP);
    }
}

/// The provider check as Gentle Shell's own model registry answers it
/// (`get_auth_status` of the active agent tab), over what the files and
/// environment suggested: signed in when it has credentials for any
/// provider; otherwise missing, keeping the look's own fix when it already
/// said so and offering the Providers screen when it guessed otherwise.
pub(super) fn with_registry(checks: &mut [Check], answer: &Result<Detail, Detail>) {
    for check in checks.iter_mut().filter(|c| c.id == CheckId::Provider) {
        match answer {
            Ok(found) => {
                check.status = Status::Ok(found.clone());
                check.fix = Fix::None;
            }
            Err(missing) => {
                if is_ok(check) {
                    check.fix = Fix::SignIn;
                }
                check.status = Status::Missing(missing.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{
        Check, CheckId, Fix, LOG_KEEP, Need, Status, Tone, all_set, install_plan, push_lines,
        required_ok, tone, usable, with_registry,
    };

    /// The registry's answer replaces the files' guess for the provider
    /// check only.
    #[test]
    fn the_registry_decides_the_provider_check() {
        let look = || {
            vec![
                ok(CheckId::Node, Need::Required),
                check(
                    CheckId::Provider,
                    Need::Optional,
                    Status::Missing("No provider signed in".into()),
                    Fix::Terminal("gentle-shell".into()),
                ),
            ]
        };
        let mut signed_in = look();
        with_registry(&mut signed_in, &Ok("Signed in: anthropic".into()));
        assert_eq!(
            signed_in[1].status,
            Status::Ok("Signed in: anthropic".into())
        );
        assert_eq!(signed_in[1].fix, Fix::None);
        assert_eq!(signed_in[0].status, Status::Ok("1".into()));

        let mut none = look();
        with_registry(&mut none, &Err("No provider signed in".into()));
        assert_eq!(
            none[1].fix,
            Fix::Terminal("gentle-shell".into()),
            "its own fix"
        );
        let mut stale = vec![ok(CheckId::Provider, Need::Optional)];
        with_registry(&mut stale, &Err("No provider signed in".into()));
        assert_eq!(
            (&stale[0].status, &stale[0].fix),
            (
                &Status::Missing("No provider signed in".into()),
                &Fix::SignIn
            )
        );
    }

    fn check(id: CheckId, need: Need, status: Status, fix: Fix) -> Check {
        Check {
            id,
            title: "t",
            summary: "s",
            need,
            status,
            fix,
        }
    }

    fn ok(id: CheckId, need: Need) -> Check {
        check(id, need, Status::Ok("1".into()), Fix::None)
    }

    #[test]
    fn plan_runs_needed_unattended_fixes_in_order() {
        let checks = vec![
            ok(CheckId::Node, Need::Required),
            check(
                CheckId::GentleShell,
                Need::Required,
                Status::Missing("not found".into()),
                Fix::Auto,
            ),
            check(
                CheckId::Setup,
                Need::Required,
                Status::Failed("exit 1".into()),
                Fix::Auto,
            ),
            check(
                CheckId::Provider,
                Need::Required,
                Status::Missing("none".into()),
                Fix::SignIn,
            ),
            check(
                CheckId::Git,
                Need::Recommended,
                Status::Missing("none".into()),
                Fix::Terminal("sudo apt install git".into()),
            ),
            check(
                CheckId::Engram,
                Need::Optional,
                Status::Missing("none".into()),
                Fix::Auto,
            ),
        ];
        assert_eq!(
            install_plan(&checks),
            VecDeque::from([CheckId::GentleShell, CheckId::Setup])
        );
    }

    #[test]
    fn readiness_ignores_extras() {
        let mut checks = vec![
            ok(CheckId::Node, Need::Required),
            ok(CheckId::GentleShell, Need::Required),
            check(
                CheckId::Engram,
                Need::Optional,
                Status::Missing("none".into()),
                Fix::Auto,
            ),
        ];
        assert!(required_ok(&checks));
        assert!(all_set(&checks));
        assert!(usable(&checks));
        checks.push(check(
            CheckId::Git,
            Need::Recommended,
            Status::Outdated("2.0".into()),
            Fix::None,
        ));
        assert!(required_ok(&checks));
        assert!(!all_set(&checks));
        checks.push(check(
            CheckId::Provider,
            Need::Required,
            Status::Missing("none".into()),
            Fix::SignIn,
        ));
        // Signing in can wait: Continue doesn't, and gentle-shell can start.
        assert!(required_ok(&checks));
        assert!(usable(&checks));
        checks.push(check(
            CheckId::GentleShell,
            Need::Required,
            Status::Missing("none".into()),
            Fix::Auto,
        ));
        assert!(!required_ok(&checks));
    }

    #[test]
    fn tones_follow_need_and_status() {
        let missing = |need| check(CheckId::Git, need, Status::Missing("x".into()), Fix::None);
        assert_eq!(tone(&missing(Need::Required)), Tone::Bad);
        // Signing in is required to chat but can wait.
        assert_eq!(
            tone(&check(
                CheckId::Provider,
                Need::Required,
                Status::Missing("x".into()),
                Fix::SignIn
            )),
            Tone::Warn
        );
        assert_eq!(tone(&missing(Need::Recommended)), Tone::Warn);
        assert_eq!(tone(&missing(Need::Optional)), Tone::Warn);
        assert_eq!(
            tone(&check(
                CheckId::Node,
                Need::Required,
                Status::Outdated("v20".into()),
                Fix::Auto
            )),
            Tone::Warn
        );
        assert_eq!(
            tone(&check(
                CheckId::Engram,
                Need::Optional,
                Status::Failed("x".into()),
                Fix::Auto
            )),
            Tone::Bad
        );
        assert_eq!(tone(&ok(CheckId::Node, Need::Required)), Tone::Good);
    }

    #[test]
    fn log_keeps_non_empty_lines_and_the_tail() {
        let mut log = Vec::new();
        push_lines(&mut log, "a\n\n  \nb  \r\nc");
        assert_eq!(log, ["a", "b", "c"]);
        for i in 0..LOG_KEEP {
            push_lines(&mut log, &i.to_string());
        }
        assert_eq!(log.len(), LOG_KEEP);
        assert_eq!(log.last().map(String::as_str), Some("199"));
    }
}
