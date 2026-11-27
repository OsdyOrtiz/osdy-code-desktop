//! The Changes tab: the work tree with staged and unstaged sides apart, and
//! what acts on it (stage, unstage, discard, stash, commit, push), plus the
//! last commit message and the staged diff.

use std::path::Path;

use gentle_core::git::status::{EntryKind, StatusEntry, WorkTree, parse_status_v2};
use gentle_core::git::truncate_diff;

use super::{GitResult, git, run};

/// The work tree of the repository at `toplevel`, with each entry's staged
/// and unstaged line counts; `None` outside one.
pub fn work_tree(toplevel: &Path) -> Option<WorkTree> {
    let mut tree = git(
        toplevel,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
    )
    .map(|raw| parse_status_v2(&raw))?;
    let numstat = |cached: bool| {
        let mut args = vec!["--no-optional-locks", "diff", "--numstat", "-z"];
        if cached {
            args.push("--cached");
        } else {
            // Against the work tree, `diff` rewrites a stat-dirty index even
            // without optional locks.
            args.splice(0..0, ["-c", "diff.autoRefreshIndex=false"]);
        }
        git(toplevel, &args).unwrap_or_default()
    };
    tree.apply_numstat(&numstat(true), &numstat(false));
    Some(tree)
}

/// Run `args` on `paths`, which are file names from `git status`: taken
/// literally, so a name with `[`, `*` or `?` never matches other files.
fn run_with(cwd: &Path, args: &[&str], paths: &[String]) -> GitResult<()> {
    let mut all: Vec<&str> = vec!["--literal-pathspecs"];
    all.extend(args);
    all.push("--");
    all.extend(paths.iter().map(String::as_str));
    run(cwd, &all).map(drop)
}

fn has_head(toplevel: &Path) -> bool {
    git(toplevel, &["rev-parse", "--verify", "-q", "HEAD"]).is_some()
}

pub fn stage(toplevel: &Path, paths: &[String]) -> GitResult<()> {
    run_with(toplevel, &["add", "-A"], paths)
}

pub fn unstage(toplevel: &Path, paths: &[String]) -> GitResult<()> {
    if has_head(toplevel) {
        run_with(toplevel, &["restore", "--staged"], paths)
    } else {
        run_with(toplevel, &["rm", "--cached", "-r", "-q"], paths)
    }
}

/// Stage every tracked file's edits; untracked files stay out.
pub fn stage_tracked(toplevel: &Path) -> GitResult<()> {
    run(toplevel, &["add", "-u"]).map(drop)
}

pub fn unstage_all(toplevel: &Path) -> GitResult<()> {
    if has_head(toplevel) {
        run(toplevel, &["reset", "-q"]).map(drop)
    } else {
        run(toplevel, &["rm", "--cached", "-r", "-q", "--", "."]).map(drop)
    }
}

/// Throw away one side of an entry. Unstaged: the worktree goes back to the
/// index (untracked files are deleted). Staged: index and worktree go back
/// to HEAD (a staged new file is deleted).
pub fn discard(toplevel: &Path, entry: &StatusEntry, staged: bool) -> GitResult<()> {
    let path = vec![entry.path.clone()];
    if !staged {
        return match entry.kind {
            EntryKind::Untracked => run_with(toplevel, &["clean", "-f", "-q"], &path),
            _ => run_with(toplevel, &["restore", "--worktree"], &path),
        };
    }
    match (entry.index, &entry.orig) {
        ('A' | 'C', _) => run_with(toplevel, &["rm", "-f", "-q"], &path),
        ('R', Some(orig)) => {
            run_with(toplevel, &["rm", "-f", "-q"], &path)?;
            let orig = vec![orig.clone()];
            run_with(
                toplevel,
                &["restore", "--source=HEAD", "--staged", "--worktree"],
                &orig,
            )
        }
        _ => run_with(
            toplevel,
            &["restore", "--source=HEAD", "--staged", "--worktree"],
            &path,
        ),
    }
}

/// How many stashes the repository keeps.
pub fn stash_count(toplevel: &Path) -> usize {
    git(
        toplevel,
        &["--no-optional-locks", "stash", "list", "--format=%gd"],
    )
    .map_or(0, |out| out.lines().count())
}

/// Stash every change, untracked files included.
pub fn stash_push(toplevel: &Path, message: &str) -> GitResult<()> {
    let mut args = vec!["stash", "push", "--include-untracked"];
    if !message.trim().is_empty() {
        args.extend(["-m", message.trim()]);
    }
    run(toplevel, &args).map(drop)
}

/// `git commit`; an amend with an empty summary keeps the old message.
pub fn commit(toplevel: &Path, summary: &str, description: &str, amend: bool) -> GitResult<()> {
    let (summary, description) = (summary.trim(), description.trim());
    let mut args = vec!["commit"];
    if amend {
        args.push("--amend");
    }
    if summary.is_empty() {
        if !amend {
            return Err("Write a summary first".into());
        }
        args.push("--no-edit");
    } else {
        args.extend(["-m", summary]);
        if !description.is_empty() {
            args.extend(["-m", description]);
        }
    }
    run(toplevel, &args).map(drop)
}

/// `git push`, publishing the branch to a remote first when it has no
/// upstream (`origin` if present, else the only/first remote).
pub fn push(toplevel: &Path) -> GitResult<()> {
    if git(toplevel, &["rev-parse", "--abbrev-ref", "@{upstream}"]).is_some() {
        return run(toplevel, &["push"]).map(drop);
    }
    let remotes = run(toplevel, &["remote"])?;
    let remotes: Vec<&str> = remotes
        .lines()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect();
    let remote = remotes
        .iter()
        .find(|r| **r == "origin")
        .or(remotes.first())
        .ok_or("This repository has no remote to push to")?;
    run(toplevel, &["push", "--set-upstream", *remote, "HEAD"]).map(drop)
}

/// The last commit's message as `(summary, description)`.
pub fn last_commit_message(toplevel: &Path) -> Option<(String, String)> {
    let message = git(toplevel, &["log", "-1", "--format=%B"])?;
    let message = message.trim();
    let (summary, description) = message.split_once('\n').unwrap_or((message, ""));
    Some((summary.trim().to_string(), description.trim().to_string()))
}

/// The staged diff with a stat header, cut to `max` bytes on a line break.
pub fn staged_diff(toplevel: &Path, max: usize) -> GitResult<String> {
    let diff = run(
        toplevel,
        &[
            "diff",
            "--cached",
            "--no-color",
            "--no-ext-diff",
            "--stat",
            "--patch",
        ],
    )?;
    Ok(truncate_diff(&diff, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh repository under the temp folder, named after `test`: the
    /// clock alone can repeat across tests running at once (macOS keeps
    /// microseconds), and two `git init`s in one folder collide.
    fn repo(test: &str) -> Option<std::path::PathBuf> {
        pi_rpc::command("git").arg("--version").output().ok()?;
        let dir = std::env::temp_dir().join(format!(
            "gentle-git-changes-{test}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q", "-b", "main"]).unwrap();
        Some(dir)
    }

    /// The clean Changes view offers the stashes only while some exist.
    #[test]
    fn stash_count_follows_pushes() {
        let Some(dir) = repo("stash-count") else {
            return;
        };
        run(&dir, &["config", "user.name", "Test"]).unwrap();
        run(&dir, &["config", "user.email", "test@example.com"]).unwrap();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        run(&dir, &["add", "a.txt"]).unwrap();
        run(
            &dir,
            &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"],
        )
        .unwrap();
        assert_eq!(stash_count(&dir), 0);

        for text in ["two\n", "three\n"] {
            std::fs::write(dir.join("a.txt"), text).unwrap();
            stash_push(&dir, "").unwrap();
        }

        let count = stash_count(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(count, 2);
    }

    /// A file name is a name, not a pattern: discarding `notes[1].txt`
    /// leaves `notes1.txt`, which the brackets would match as a glob.
    #[test]
    fn discarding_an_untracked_file_leaves_the_files_its_name_matches_as_a_glob() {
        let Some(dir) = repo("discard") else { return };
        for name in ["notes[1].txt", "notes1.txt"] {
            std::fs::write(dir.join(name), "text\n").unwrap();
        }
        let tree = work_tree(&dir).unwrap();
        let entry = tree
            .entries
            .iter()
            .find(|entry| entry.path == "notes[1].txt")
            .unwrap();

        discard(&dir, entry, false).unwrap();

        assert!(!dir.join("notes[1].txt").exists());
        assert!(
            dir.join("notes1.txt").exists(),
            "discarding one file deleted another"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Staging takes the same literal names.
    #[test]
    fn staging_a_file_leaves_the_files_its_name_matches_as_a_glob() {
        let Some(dir) = repo("stage") else { return };
        for name in ["notes[1].txt", "notes1.txt"] {
            std::fs::write(dir.join(name), "text\n").unwrap();
        }

        stage(&dir, &["notes[1].txt".to_string()]).unwrap();

        let tree = work_tree(&dir).unwrap();
        let kind = |path: &str| {
            let entry = tree.entries.iter().find(|entry| entry.path == path);
            entry.unwrap().kind
        };
        assert_eq!(kind("notes[1].txt"), EntryKind::Tracked);
        assert_eq!(kind("notes1.txt"), EntryKind::Untracked);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Changes tab polls `work_tree` every few seconds: it must not
    /// rewrite `.git/index` (and take `index.lock`) under the user's own git.
    #[test]
    fn work_tree_leaves_a_stat_dirty_index_unwritten() {
        let Some(dir) = super::super::tests::stat_dirty_repo("work-tree") else {
            return;
        };
        let index = std::fs::read(dir.join(".git/index")).unwrap();

        let tree = work_tree(&dir).unwrap();

        let unchanged = std::fs::read(dir.join(".git/index")).unwrap() == index;
        let _ = std::fs::remove_dir_all(&dir);
        let entries: Vec<_> = tree
            .entries
            .iter()
            .map(|e| {
                let sides = (e.staged(), e.unstaged());
                (e.path.as_str(), sides, e.staged_lines, e.unstaged_lines)
            })
            .collect();
        assert_eq!(
            entries,
            vec![
                ("edit.txt", (None, Some('M')), None, Some((2, 1))),
                ("new.txt", (None, Some('?')), None, None),
            ]
        );
        assert!(unchanged, "work_tree rewrote .git/index");
    }
}
