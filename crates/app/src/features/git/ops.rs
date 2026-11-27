//! The Git pane's actions on the repository: fetch, pull, branches and tags
//! at a commit, checkout, cherry-pick, revert, reset, rewording HEAD,
//! remotes, Git LFS, and finishing or abandoning a command git stopped in.
//! Each returns git's own message on failure.

use std::path::Path;

use gentle_core::git::ops::{
    CommitFile, InProgress, PullMode, ResetMode, diff_tree_args, parse_commit_files,
};

use super::{GitResult, Text, git, run};

/// Fetch every remote, dropping remote branches deleted there.
pub fn fetch_all(toplevel: &Path) -> GitResult<()> {
    run(toplevel, &["fetch", "--all", "--prune"]).map(drop)
}

pub fn pull(toplevel: &Path, mode: PullMode) -> GitResult<()> {
    run(toplevel, mode.args()).map(drop)
}

/// A new branch at `at`; `checkout` switches to it as well.
pub fn create_branch(toplevel: &Path, name: &str, at: &str, checkout: bool) -> GitResult<()> {
    let name = valid_name(toplevel, name, true)?;
    if checkout {
        run(toplevel, &["switch", "-c", &name, at]).map(drop)
    } else {
        run(toplevel, &["branch", "--", &name, at]).map(drop)
    }
}

/// A lightweight tag at `at`, or an annotated one when `message` isn't blank.
pub fn create_tag(toplevel: &Path, name: &str, at: &str, message: &str) -> GitResult<()> {
    let name = valid_name(toplevel, name, false)?;
    let message = message.trim();
    if message.is_empty() {
        run(toplevel, &["tag", "--", &name, at]).map(drop)
    } else {
        run(toplevel, &["tag", "-a", "-m", message, "--", &name, at]).map(drop)
    }
}

/// `name` trimmed, once git accepts it as a branch (or tag) name.
fn valid_name(toplevel: &Path, name: &str, branch: bool) -> GitResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Write a name first".into());
    }
    let ok = if branch {
        git(toplevel, &["check-ref-format", "--branch", name]).is_some()
    } else {
        git(
            toplevel,
            &["check-ref-format", &format!("refs/tags/{name}")],
        )
        .is_some()
    };
    if ok {
        Ok(name.to_string())
    } else {
        Err(Text::from("“{name}” isn't a valid name").with("{name}", name))
    }
}

/// Switch to a local branch.
pub fn switch(toplevel: &Path, branch: &str) -> GitResult<()> {
    run(toplevel, &["switch", branch]).map(drop)
}

/// Check out a commit on a detached HEAD.
pub fn checkout_detached(toplevel: &Path, hash: &str) -> GitResult<()> {
    run(toplevel, &["switch", "--detach", hash]).map(drop)
}

/// Apply `hash`'s changes as a new commit on HEAD; a merge takes its first
/// parent's side.
pub fn cherry_pick(toplevel: &Path, hash: &str, merge: bool) -> GitResult<()> {
    let mut args = vec!["cherry-pick"];
    if merge {
        args.extend(["-m", "1"]);
    }
    args.push(hash);
    run(toplevel, &args).map(drop)
}

/// A new commit undoing `hash`; a merge is undone against its first parent.
pub fn revert(toplevel: &Path, hash: &str, merge: bool) -> GitResult<()> {
    let mut args = vec!["revert", "--no-edit"];
    if merge {
        args.extend(["-m", "1"]);
    }
    args.push(hash);
    run(toplevel, &args).map(drop)
}

/// Move the current branch to `hash`.
pub fn reset(toplevel: &Path, hash: &str, mode: ResetMode) -> GitResult<()> {
    run(toplevel, &["reset", mode.flag(), hash]).map(drop)
}

/// Replace HEAD's message, leaving its changes (and the index) alone.
pub fn reword_head(toplevel: &Path, message: &str) -> GitResult<()> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Write a message first".into());
    }
    run(toplevel, &["commit", "--amend", "--only", "-m", message]).map(drop)
}

/// A commit's full message.
pub fn message(toplevel: &Path, hash: &str) -> Option<String> {
    git(toplevel, &["log", "-1", "--format=%B", hash]).map(|m| m.trim_end().to_string())
}

/// The files `hash` changed against its first parent.
pub fn commit_files(toplevel: &Path, hash: &str, parent: Option<&str>) -> Vec<CommitFile> {
    let read = |kind| git(toplevel, &diff_tree_args(kind, hash, parent)).unwrap_or_default();
    parse_commit_files(&read("--name-status"), &read("--numstat"))
}

pub fn add_remote(toplevel: &Path, name: &str, url: &str) -> GitResult<()> {
    let (name, url) = (name.trim(), url.trim());
    if name.is_empty() || url.is_empty() {
        return Err("Write the remote's name and URL".into());
    }
    run(toplevel, &["remote", "add", "--", name, url]).map(drop)
}

/// Push one tag to `remote`.
pub fn push_tag(toplevel: &Path, remote: &str, tag: &str) -> GitResult<()> {
    run(toplevel, &["push", remote, &format!("refs/tags/{tag}")]).map(drop)
}

pub fn delete_tag(toplevel: &Path, tag: &str) -> GitResult<()> {
    run(toplevel, &["tag", "-d", "--", tag]).map(drop)
}

/// Git LFS is installed for this repository's git.
pub fn lfs_available(toplevel: &Path) -> bool {
    git(toplevel, &["lfs", "version"]).is_some()
}

/// The Git LFS commands the pane offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lfs {
    Checkout,
    Pull,
    Push,
    Prune,
}

impl Lfs {
    pub const ALL: [Lfs; 4] = [Lfs::Checkout, Lfs::Pull, Lfs::Push, Lfs::Prune];

    pub fn label(self) -> &'static str {
        match self {
            Lfs::Checkout => "LFS checkout",
            Lfs::Pull => "LFS pull",
            Lfs::Push => "LFS push",
            Lfs::Prune => "LFS prune",
        }
    }
}

pub fn lfs(toplevel: &Path, op: Lfs, remote: Option<&str>) -> GitResult<()> {
    match op {
        Lfs::Checkout => run(toplevel, &["lfs", "checkout"]),
        Lfs::Pull => run(toplevel, &["lfs", "pull"]),
        Lfs::Push => {
            let remote = remote.ok_or("This repository has no remote to push to")?;
            run(toplevel, &["lfs", "push", "--all", remote])
        }
        Lfs::Prune => run(toplevel, &["lfs", "prune"]),
    }
    .map(drop)
}

pub fn continue_in_progress(toplevel: &Path, op: InProgress) -> GitResult<()> {
    run(toplevel, &op.continue_args()).map(drop)
}

pub fn abort_in_progress(toplevel: &Path, op: InProgress) -> GitResult<()> {
    run(toplevel, &op.abort_args()).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> Option<std::path::PathBuf> {
        pi_rpc::command("git").arg("--version").output().ok()?;
        let dir = std::env::temp_dir().join(format!(
            "gentle-git-ops-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| run(&dir, args).unwrap();
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);
        Some(dir)
    }

    fn commit(dir: &Path, file: &str, text: &str, subject: &str) -> String {
        std::fs::write(dir.join(file), text).unwrap();
        run(dir, &["add", "--", file]).unwrap();
        run(dir, &["commit", "-q", "-m", subject]).unwrap();
        run(dir, &["rev-parse", "HEAD"]).unwrap().trim().to_string()
    }

    /// A conflicting cherry-pick is detected and can be abandoned; the
    /// other commit actions land where they should.
    #[test]
    fn commit_actions_against_a_temp_repo() {
        let Some(dir) = repo() else { return };
        let base = commit(&dir, "a.txt", "one\n", "base");
        let second = commit(&dir, "a.txt", "two\n", "second");

        // Branch and tag at the first commit; names git refuses fail early.
        create_branch(&dir, "side", &base, true).unwrap();
        assert!(create_branch(&dir, "bad name", &base, false).is_err());
        assert!(create_tag(&dir, "  ", &base, "").is_err());
        create_tag(&dir, "v1", &base, "first release").unwrap();
        assert_eq!(
            run(&dir, &["cat-file", "-t", "v1"]).unwrap().trim(),
            "tag",
            "a message makes an annotated tag"
        );

        // On `side`, picking `second` conflicts with a change of its own.
        commit(&dir, "a.txt", "three\n", "side change");
        assert!(cherry_pick(&dir, &second, false).is_err());
        assert_eq!(
            super::super::log::in_progress(&dir),
            Some(InProgress::CherryPick)
        );
        abort_in_progress(&dir, InProgress::CherryPick).unwrap();
        assert_eq!(super::super::log::in_progress(&dir), None);

        // Revert makes a commit without an editor; reset moves the branch back.
        let before = run(&dir, &["rev-parse", "HEAD"]).unwrap();
        revert(&dir, before.trim(), false).unwrap();
        reset(&dir, before.trim(), ResetMode::DiscardChanges).unwrap();
        assert_eq!(run(&dir, &["rev-parse", "HEAD"]).unwrap(), before);

        reword_head(&dir, "side change, reworded").unwrap();
        assert_eq!(
            message(&dir, "HEAD").as_deref(),
            Some("side change, reworded")
        );

        let files = commit_files(&dir, &second, Some(&base));
        assert_eq!(files.len(), 1);
        assert_eq!(
            (files[0].status, files[0].added, files[0].removed),
            ('M', Some(1), Some(1))
        );
        // The root commit compares with nothing.
        assert_eq!(commit_files(&dir, &base, None)[0].status, 'A');

        let _ = std::fs::remove_dir_all(&dir);
    }
}
