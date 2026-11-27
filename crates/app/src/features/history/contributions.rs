//! The user's GitHub contribution calendar for the Git pane's activity
//! strip: every repository, pull requests, issues and reviews, as their
//! profile shows it. Call off the UI thread.
//!
//! The account comes from `git config github.user`, a GitHub noreply
//! `user.email`, the account GitHub ties to `user.email` in this repository
//! (its commits API, when the main remote is on GitHub), or the account the
//! GitHub CLI is signed in with. The calendar is the profile's public page
//! (`github.com/users/<login>/contributions`); private contributions count
//! when the user lets their profile show them.

use std::path::Path;
use std::time::Duration;

use gentle_core::git::activity::{
    contributions_url, noreply_login, parse_contributions, valid_login,
};
use gentle_core::git::avatar::{github_commits_url, github_repo, parse_commits_login};

use super::avatars::{Answer, get};
use crate::features::{files::Text, git};

pub struct Contributions {
    pub login: String,
    /// Contributions per day (days since 1970-01-01).
    pub days: Vec<(i64, u32)>,
}

/// The calendar, or why it can't be shown. `github` is the repository's
/// `(owner, repo)` when known; otherwise `origin` is read for it.
pub fn read(toplevel: &Path, github: Option<(String, String)>) -> Result<Contributions, Text> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let github = github
        .or_else(|| git::config(toplevel, "remote.origin.url").and_then(|url| github_repo(&url)));
    let login = login(toplevel, github.as_ref(), &agent).ok_or(
        "Couldn't tell your GitHub account. Set it with \
         `git config --global github.user <login>`.",
    )?;
    match get(&agent, &contributions_url(&login), false) {
        Answer::Body(html) => {
            let days = parse_contributions(&String::from_utf8_lossy(&html));
            if days.is_empty() {
                Err(Text::from("GitHub showed no calendar for @{login}.").with("{login}", login))
            } else {
                Ok(Contributions { login, days })
            }
        }
        Answer::NotFound => {
            Err(Text::from("GitHub has no account @{login}.").with("{login}", login))
        }
        Answer::Refused => Err("GitHub is limiting requests; try again in a while.".into()),
        Answer::Offline => Err("Couldn't reach GitHub.".into()),
    }
}

fn login(
    toplevel: &Path,
    github: Option<&(String, String)>,
    agent: &ureq::Agent,
) -> Option<String> {
    if let Some(login) = git::config(toplevel, "github.user").filter(|l| valid_login(l)) {
        return Some(login);
    }
    let email = git::config(toplevel, "user.email");
    if let Some(login) = email.as_deref().and_then(noreply_login) {
        return Some(login);
    }
    if let (Some(email), Some((owner, repo))) = (&email, github)
        && let Answer::Body(json) = get(agent, &github_commits_url(owner, repo, email), true)
        && let Some(login) = parse_commits_login(&String::from_utf8_lossy(&json))
    {
        return Some(login);
    }
    // The GitHub CLI's signed-in account, last: with several, it may not be
    // the one this repository's commits are made with.
    let out = pi_rpc::command("gh")
        .args(["api", "user", "--jq", ".login"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let login = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && valid_login(&login)).then_some(login)
}
