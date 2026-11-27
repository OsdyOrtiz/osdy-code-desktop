//! Where agent tabs run: this computer, a WSL2 distro, or any environment a
//! command reaches (a Linux VM through OrbStack or Lima, a container). The
//! app stays the native frontend; `gentle-shell --mode rpc` runs in the
//! backend and speaks the same JSONL over the launching command's stdio.
//!
//! A backend is two things: the command that runs a program in a folder
//! there ([`Backend::launch`]), and how a path on this computer reads there
//! ([`Backend::path`]). The choice is the app's `settings.json` key
//! [`KEY`]; a folder under `\\wsl.localhost\<distro>\` implies that distro
//! ([`Backend::for_folder`]).

use serde_json::{Value, json};

use crate::i18n::{Language, tr};

/// The app's `settings.json` key.
pub const KEY: &str = "backend";

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum Backend {
    /// This computer, as before.
    #[default]
    Local,
    /// A WSL2 distro, through `wsl.exe`.
    Wsl { distro: String },
    /// Any environment a command prefix reaches, given as argv
    /// (`orb -m dev --`, `limactl shell dev`, `docker exec -i dev`). It sees
    /// this computer's paths as they are, as OrbStack and Lima mount them.
    Command { prefix: Vec<String> },
}

impl Backend {
    /// `{"kind": "wsl", "distro": "Ubuntu"}` or `{"kind": "command",
    /// "prefix": ["orb", "-m", "dev", "--"]}`; anything else, a blank distro
    /// or an empty prefix is [`Backend::Local`].
    pub fn from_json(value: &Value) -> Self {
        match value["kind"].as_str() {
            Some("wsl") => match value["distro"].as_str().map(str::trim) {
                Some(distro) if !distro.is_empty() => Self::Wsl {
                    distro: distro.to_string(),
                },
                _ => Self::Local,
            },
            Some("command") => {
                let prefix: Vec<String> = value["prefix"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|arg| arg.as_str().map(str::to_string))
                    .collect();
                if prefix
                    .first()
                    .is_some_and(|program| !program.trim().is_empty())
                {
                    Self::Command { prefix }
                } else {
                    Self::Local
                }
            }
            _ => Self::Local,
        }
    }

    /// The value [`Self::from_json`] reads back; `None` for
    /// [`Backend::Local`], whose key is removed.
    pub fn to_json(&self) -> Option<Value> {
        match self {
            Self::Local => None,
            Self::Wsl { distro } => Some(json!({"kind": "wsl", "distro": distro})),
            Self::Command { prefix } => Some(json!({"kind": "command", "prefix": prefix})),
        }
    }

    /// The distro a folder under `\\wsl.localhost\<distro>\` (or the older
    /// `\\wsl$\<distro>\`) lives in: a project opened from there runs there.
    pub fn for_folder(folder: &str) -> Option<Self> {
        let (distro, _) = wsl_share(folder)?;
        Some(Self::Wsl {
            distro: distro.to_string(),
        })
    }

    /// How the backend sees `host`, a path on this computer: unchanged
    /// locally and through a command; in WSL, `C:\a` is `/mnt/c/a` and
    /// `\\wsl.localhost\<distro>\home\a` is `/home/a`. `None` when the
    /// backend can't reach it (another distro's share, a network path).
    pub fn path(&self, host: &str) -> Option<String> {
        match self {
            Self::Local | Self::Command { .. } => Some(host.to_string()),
            Self::Wsl { distro } => {
                if let Some((share, rest)) = wsl_share(host) {
                    return share
                        .eq_ignore_ascii_case(distro)
                        .then(|| format!("/{}", rest.replace('\\', "/")));
                }
                // A verbatim drive path (`\\?\C:\a`) reads as `C:\a`.
                let host = host.strip_prefix(r"\\?\").unwrap_or(host);
                if host.starts_with('/') {
                    return Some(host.to_string());
                }
                let mut chars = host.chars();
                let drive = chars.next().filter(char::is_ascii_alphabetic)?;
                let rest = chars.as_str().strip_prefix(':')?;
                if !(rest.is_empty() || rest.starts_with(['\\', '/'])) {
                    return None;
                }
                let rest = rest.trim_start_matches(['\\', '/']).replace('\\', "/");
                Some(format!("/mnt/{}/{rest}", drive.to_ascii_lowercase()))
            }
        }
    }

    /// The argv that runs a program (appended after it) in `cwd`, a path as
    /// the backend sees it. `wsl.exe --exec` passes the arguments as they
    /// are, without a shell re-reading them; through a command a small
    /// `sh` changes folder first. `None` for [`Backend::Local`].
    pub fn launch(&self, cwd: &str) -> Option<Vec<String>> {
        match self {
            Self::Local => None,
            Self::Wsl { distro } => Some(
                ["wsl.exe", "-d", distro, "--cd", cwd, "--exec"]
                    .map(str::to_string)
                    .to_vec(),
            ),
            Self::Command { prefix } => {
                let mut argv = prefix.clone();
                argv.extend(
                    ["sh", "-c", r#"cd "$1" && shift && exec "$@""#, "sh", cwd].map(str::to_string),
                );
                Some(argv)
            }
        }
    }

    /// The argv of an interactive terminal in `cwd`, a path as the backend
    /// sees it, or `~` for the user's home there: the user's login shell.
    /// `wsl.exe` without `--exec` starts the distro's own (its `--cd` takes
    /// `~` too). `None` for [`Backend::Local`].
    pub fn terminal(&self, cwd: &str) -> Option<Vec<String>> {
        match self {
            Self::Local => None,
            Self::Wsl { distro } => Some(
                ["wsl.exe", "-d", distro, "--cd", cwd]
                    .map(str::to_string)
                    .to_vec(),
            ),
            Self::Command { prefix } => {
                let script = format!(
                    r#"if [ "$1" = "~" ]; then cd; else cd "$1" 2>/dev/null; fi; {LOGIN_SHELL}; exec "$s" -l"#
                );
                let mut argv = prefix.clone();
                argv.extend(["sh", "-c", &script, "sh", cwd].map(str::to_string));
                Some(argv)
            }
        }
    }

    /// How this computer sees `path`, a path the backend printed (`git
    /// rev-parse --show-toplevel`): the reverse of [`Self::path`]. In WSL,
    /// `/mnt/c/a` is `C:\a` and `/home/a` is `\\wsl.localhost\<distro>\home\a`;
    /// elsewhere it is unchanged.
    pub fn host_path(&self, path: &str) -> String {
        let Self::Wsl { distro } = self else {
            return path.to_string();
        };
        if !path.starts_with('/') {
            return path.to_string();
        }
        if let Some(rest) = path.strip_prefix("/mnt/") {
            let (drive, rest) = rest.split_once('/').unwrap_or((rest, ""));
            if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
                return format!(
                    r"{}:\{}",
                    drive.to_ascii_uppercase(),
                    rest.replace('/', r"\")
                );
            }
        }
        format!(r"\\wsl.localhost\{distro}{}", path.replace('/', r"\"))
    }

    /// A short name for the backend: "This computer", "WSL · Ubuntu",
    /// "orb -m dev --".
    pub fn label(&self) -> String {
        self.label_in(Language::English)
    }

    /// The display name in the requested interface language.
    pub fn label_in(&self, language: Language) -> String {
        match self {
            Self::Local => tr(language, "This computer").to_string(),
            Self::Wsl { distro } => format!("WSL · {distro}"),
            Self::Command { prefix } => prefix.join(" "),
        }
    }
}

/// `(distro, rest)` of `\\wsl.localhost\<distro>\<rest>` or
/// `\\wsl$\<distro>\<rest>`, either slash, also in the verbatim form
/// Windows canonicalizes it to (`\\?\UNC\wsl.localhost\…`).
fn wsl_share(path: &str) -> Option<(&str, &str)> {
    let rest = path
        .strip_prefix(r"\\?\UNC\")
        .or_else(|| path.strip_prefix(r"\\"))
        .or_else(|| path.strip_prefix("//"))?;
    let (host, rest) = rest.split_once(['\\', '/'])?;
    if !(host.eq_ignore_ascii_case("wsl.localhost") || host.eq_ignore_ascii_case("wsl$")) {
        return None;
    }
    let (distro, rest) = rest.split_once(['\\', '/']).unwrap_or((rest, ""));
    (!distro.is_empty()).then_some((distro, rest))
}

const PATH_START: &str = "__GENTLE_PATH_START__";
const PATH_END: &str = "__GENTLE_PATH_END__";

/// `sh` that sets `s` to the user's login shell: `$SHELL`, else the
/// password database's, else `/bin/sh`.
const LOGIN_SHELL: &str = r#"s="${SHELL:-}"; [ -n "$s" ] || s="$(getent passwd "$(id -un)" 2>/dev/null | cut -d: -f7)"; s="${s:-/bin/sh}""#;

/// The argv (after [`Backend::launch`]) that prints the backend user's login
/// PATH between markers: its login shell, interactive so the rc files that
/// set up nvm, fnm or mise run. `wsl.exe --exec` and most commands start
/// programs without a login shell, so `gentle-shell` and its Node would not
/// be found otherwise.
pub fn path_probe() -> Vec<String> {
    let script =
        format!(r#"{LOGIN_SHELL}; exec "$s" -l -i -c 'printf "{PATH_START}%s{PATH_END}" "$PATH"'"#);
    vec!["sh".to_string(), "-c".to_string(), script]
}

/// Whether `path`, as WSL prints it, is a Windows file reached through its
/// interop (`/mnt/c/…`, Windows' PATH appended to the distro's): a Windows
/// install, not the distro's own.
pub fn is_windows_file(path: &str) -> bool {
    path.strip_prefix("/mnt/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(drive, _)| {
            drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic())
        })
}

/// The PATH [`path_probe`] printed, fenced from anything the rc files wrote.
pub fn parse_path_probe(out: &str) -> Option<String> {
    let start = out.find(PATH_START)? + PATH_START.len();
    let end = out[start..].find(PATH_END)? + start;
    let path = out[start..end].trim();
    (!path.is_empty()).then(|| path.to_string())
}

/// One distro of `wsl.exe --list --verbose`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub name: String,
    /// The default distro (marked `*`).
    pub default: bool,
    /// WSL 2 (WSL 1 has no real Linux kernel; it works, slower).
    pub wsl2: bool,
}

/// The distros in `wsl.exe --list --verbose` output: a header, then
/// `[*] NAME STATE VERSION` per line. `wsl.exe` writes UTF-16 unless
/// `WSL_UTF8=1`, so NULs are dropped first.
pub fn parse_distros(out: &str) -> Vec<Distro> {
    let out = out.replace(['\0', '\u{feff}'], "");
    out.lines()
        .skip(1)
        .filter_map(|line| {
            let line = line.trim();
            let (default, line) = match line.strip_prefix('*') {
                Some(rest) => (true, rest.trim_start()),
                None => (false, line),
            };
            let mut fields: Vec<&str> = line.split_whitespace().collect();
            let version = fields.pop()?;
            fields.pop()?;
            let name = fields.join(" ");
            (!name.is_empty()).then(|| Distro {
                name,
                default,
                wsl2: version == "2",
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wsl() -> Backend {
        Backend::Wsl {
            distro: "Ubuntu".into(),
        }
    }

    #[test]
    fn settings_round_trip_and_fall_back_to_local() {
        for backend in [
            wsl(),
            Backend::Command {
                prefix: vec!["orb".into(), "-m".into(), "dev".into(), "--".into()],
            },
        ] {
            assert_eq!(Backend::from_json(&backend.to_json().unwrap()), backend);
        }
        assert_eq!(Backend::Local.to_json(), None);
        for value in [
            json!(null),
            json!({"kind": "wsl", "distro": "  "}),
            json!({"kind": "command", "prefix": []}),
            json!({"kind": "command", "prefix": [""]}),
            json!({"kind": "ssh"}),
        ] {
            assert_eq!(Backend::from_json(&value), Backend::Local, "{value}");
        }
    }

    #[test]
    fn a_folder_in_a_wsl_share_runs_in_that_distro() {
        for folder in [
            r"\\wsl.localhost\Ubuntu\home\ana\api",
            r"\\wsl$\Ubuntu\home\ana\api",
            "//wsl.localhost/Ubuntu/home/ana/api",
            // What `canonicalize` makes of a share on Windows.
            r"\\?\UNC\wsl.localhost\Ubuntu\home\ana\api",
            r"\\?\UNC\wsl$\Ubuntu\home\ana\api",
        ] {
            assert_eq!(Backend::for_folder(folder), Some(wsl()), "{folder}");
        }
        assert_eq!(Backend::for_folder(r"C:\Users\ana\api"), None);
        assert_eq!(Backend::for_folder(r"\\server\share\api"), None);
        assert_eq!(Backend::for_folder("/home/ana/api"), None);
    }

    #[test]
    fn windows_installs_seen_through_wsl_interop_are_told_apart() {
        assert!(is_windows_file(
            "/mnt/c/Users/ana/AppData/Local/pnpm/gentle-shell"
        ));
        assert!(!is_windows_file("/usr/local/bin/gentle-shell"));
        assert!(!is_windows_file(
            "/home/ana/.nvm/versions/node/v22/bin/gentle-shell"
        ));
        assert!(!is_windows_file("/mnt/wsl/shared/gentle-shell"));
        assert!(!is_windows_file("/mnt/mac/Users/ana/gentle-shell"));
    }

    #[test]
    fn wsl_reads_windows_and_share_paths_as_linux_paths() {
        let wsl = wsl();
        let cases = [
            (r"C:\Users\Ana Li\api", Some("/mnt/c/Users/Ana Li/api")),
            ("D:/data", Some("/mnt/d/data")),
            (r"C:\", Some("/mnt/c/")),
            (r"\\?\C:\Users\ana", Some("/mnt/c/Users/ana")),
            (
                r"\\wsl.localhost\Ubuntu\home\ana\api",
                Some("/home/ana/api"),
            ),
            (r"\\wsl$\ubuntu\home\ana", Some("/home/ana")),
            (
                r"\\?\UNC\wsl.localhost\Ubuntu\home\ana\api",
                Some("/home/ana/api"),
            ),
            (r"\\?\UNC\server\share\api", None),
            (r"\\wsl.localhost\Ubuntu", Some("/")),
            ("/home/ana/api", Some("/home/ana/api")),
            (r"\\wsl.localhost\Debian\home\ana", None),
            (r"\\server\share\api", None),
            ("relative\\path", None),
            ("C:relative", None),
        ];
        for (host, linux) in cases {
            assert_eq!(wsl.path(host).as_deref(), linux, "{host}");
        }
        let orb = Backend::Command {
            prefix: vec!["orb".into()],
        };
        assert_eq!(
            orb.path("/Users/ana/api").as_deref(),
            Some("/Users/ana/api")
        );
    }

    #[test]
    fn launch_runs_the_program_in_the_folder() {
        assert_eq!(Backend::Local.launch("/x"), None);
        assert_eq!(
            wsl().launch("/home/ana/api").unwrap(),
            ["wsl.exe", "-d", "Ubuntu", "--cd", "/home/ana/api", "--exec"]
        );
        let orb = Backend::Command {
            prefix: vec!["orb".into(), "-m".into(), "dev".into(), "--".into()],
        };
        let argv = orb.launch("/Users/ana/my api").unwrap();
        assert_eq!(&argv[..4], ["orb", "-m", "dev", "--"]);
        assert_eq!(argv.last().map(String::as_str), Some("/Users/ana/my api"));
    }

    #[test]
    fn backend_paths_read_back_as_this_computers() {
        let ubuntu = wsl();
        for host in [
            r"C:\Users\ana\my api",
            r"\\wsl.localhost\Ubuntu\home\ana\api",
        ] {
            let there = ubuntu.path(host).unwrap();
            assert_eq!(ubuntu.host_path(&there), host);
        }
        assert_eq!(ubuntu.host_path("/mnt/d"), r"D:\");
        assert_eq!(
            ubuntu.host_path("/mnt/wsl/x"),
            r"\\wsl.localhost\Ubuntu\mnt\wsl\x"
        );
        assert_eq!(ubuntu.host_path("relative/file.rs"), "relative/file.rs");
        let orb = Backend::Command {
            prefix: vec!["orb".into()],
        };
        assert_eq!(orb.host_path("/Users/ana/api"), "/Users/ana/api");
    }

    /// The argv really runs the program in the folder, spaces and all.
    #[cfg(unix)]
    #[test]
    fn a_command_backend_runs_programs_in_the_folder() {
        let dir = std::env::temp_dir().join(format!("gentle backend {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let env = Backend::Command {
            prefix: vec!["env".into()],
        };
        let mut argv = env.launch(&dir.to_string_lossy()).unwrap();
        argv.push("pwd".into());
        let out = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .output()
            .unwrap();
        let _ = std::fs::remove_dir(&dir);
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            dir.to_string_lossy()
        );
    }

    #[test]
    fn the_probe_fences_the_path_from_rc_noise() {
        let out = "Welcome!\n__GENTLE_PATH_START__/home/ana/.nvm/bin:/usr/bin__GENTLE_PATH_END__";
        assert_eq!(
            parse_path_probe(out).as_deref(),
            Some("/home/ana/.nvm/bin:/usr/bin")
        );
        assert_eq!(parse_path_probe("no markers"), None);
        assert_eq!(
            parse_path_probe("__GENTLE_PATH_START____GENTLE_PATH_END__"),
            None
        );
    }

    #[test]
    fn distros_come_from_wsl_list_verbose() {
        let out = "  NAME            STATE           VERSION\n\
                   * Ubuntu-24.04    Running         2\n  \
                   Debian          Stopped         2\n  \
                   Legacy Box      Stopped         1\n";
        let utf16_like: String = out.chars().flat_map(|c| [c, '\0']).collect();
        for text in [out.to_string(), utf16_like] {
            assert_eq!(
                parse_distros(&text),
                [
                    Distro {
                        name: "Ubuntu-24.04".into(),
                        default: true,
                        wsl2: true
                    },
                    Distro {
                        name: "Debian".into(),
                        default: false,
                        wsl2: true
                    },
                    Distro {
                        name: "Legacy Box".into(),
                        default: false,
                        wsl2: false
                    },
                ]
            );
        }
    }
}
