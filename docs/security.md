# Security of contributions

Anyone can open a pull request, and what gets merged ends up on users' machines: the app runs agents, starts processes, reads configuration and replaces itself on update. This page covers how a PR is kept from carrying malicious or vulnerable code: what runs automatically, what the reviewer does, and the repository settings that make both binding.

No tool detects malicious code reliably. The checks narrow down what a person must read; **a maintainer's review is the control**, and the repository settings make sure nothing reaches `main` or a release without it.

## Where an attacker would aim

| Target | How | What stops it |
| --- | --- | --- |
| Users, through the app | Code that sends data out, runs commands, or weakens the updater | Code Owners review |
| Users, through a dependency | A new or bumped crate (typosquatting, a hijacked version, a git source) | Review of `Cargo.lock` |
| Contributors and CI, at build time | A `build.rs` or proc macro runs on every machine that compiles | Code Owners on `build.rs`/`Cargo.*`; outside contributors' workflows need approval |
| CI, to steal or publish | Workflow changes, script injection, `pull_request_target` | Read-only token, no secrets on PRs, Code Owners on `.github/` |
| Releases | Pushing a `v*` tag publishes binaries that installed apps update to | Tag ruleset: only maintainers create `v*` tags |
| Reviewers' eyes | Hidden Unicode that makes code read differently than it compiles | Review of the diff |

## Automatic checks

The repository runs no automated security scanning for now (it is a personal project with a few users): no `cargo-deny`, dependency review, `zizmor` or risk report. CI only builds and tests.

Also enforced in the workflows:

- Every workflow runs with `permissions: contents: read` unless a job needs more: `release` writes, only on tags, and `issue-claims` writes issues (assignees, labels, comments) for `/take` and `/drop`, without checking out or running code.
- PRs use `pull_request`, never `pull_request_target`: a fork's code gets no secrets and a read-only token.
- Actions are pinned to a commit SHA (a moved tag can't change what runs); they are updated by hand.
- `actions/checkout` runs with `persist-credentials: false`, so later steps can't reuse the token.

These checks run code from the PR itself, so a PR that edits `.github/` can also weaken them. That is why `.github/` has Code Owners and why a reviewer reads workflow changes before approving the run.

## Reviewing a pull request

1. **Before approving the workflow run** of a first-time or outside contributor, skim the diff: anything under `.github/`, a `build.rs`, a new dependency or a script runs on CI as soon as you approve.
2. **Dependencies**: for each new crate in `Cargo.lock`, check the name (typosquats: `serde_jsom`), the owner and download count on crates.io, and that it's needed. A bump of a crate the PR doesn't mention needs a reason. `cargo tree -i <crate>` shows who pulls it in.
3. **Processes, network, files**: a new URL, a `Command::new`, or a read outside the app's data dir must match what the PR says it does. Data leaving the machine needs an explicit reason.
4. **Binaries** (images, fonts, archives) aren't reviewable in the diff; check their origin, or regenerate them yourself (icons: `packaging/icons/generate.sh`).
5. **Obfuscation**: encoded strings, very long lines, code that builds code (`include_bytes!` of something new, `eval` in scripts) without a clear reason are reasons to reject.
6. When in doubt, ask for the change to be split, or close the PR. Suspicion of malice goes to [`SECURITY.md`](../SECURITY.md), not to a public comment.

## Repository settings (admins)

Files can't enforce these; an admin of `OsdyOrtiz/osdy-code-desktop` sets them once. Without them, the checks above are advice.

**Settings → Actions → General**

- Fork pull request workflows from outside collaborators: **Require approval for all external contributors**.
- Workflow permissions: **Read repository contents and packages permissions**; leave "Allow GitHub Actions to create and approve pull requests" off.
- Optional: "Require actions to be pinned to a full-length commit SHA".

**Settings → Rules → Rulesets → New branch ruleset** (target: default branch, enforcement: Active):

- Restrict deletions; block force pushes.
- Require a pull request before merging: 1 approval, **Require review from Code Owners**, **Dismiss stale pull request approvals when new commits are pushed**, **Require approval of the most recent reviewable push**, require conversation resolution.
- Require status checks to pass: CI's `test (macos-latest)`, `test (ubuntu-latest)`, `test (windows-latest)`. CI skips PRs that only touch Markdown or `packaging/icons/` (`paths-ignore`), which leaves its required checks pending: such a PR needs a maintainer bypass, or drop the CI checks from the list.
- Bypass list: empty, or only the release maintainers.

**Settings → Rules → Rulesets → New tag ruleset** (target: `v*`): restrict creations, updates and deletions; bypass only the people who publish releases. A `v*` tag publishes binaries that every installed app updates to.

**Settings → Code security**: Dependency graph, Secret scanning with push protection, and **Private vulnerability reporting** (what [`SECURITY.md`](../SECURITY.md) points to).

**Collaborators and organization**: write access only for people who review; require two-factor authentication for the organization. Every owner in [`.github/CODEOWNERS`](../.github/CODEOWNERS) needs write access, or GitHub ignores the line.
