//! Whether Gentle Shell's `packages` list already holds a package the app
//! installs by default, under any of the sources Pi accepts (`npm:`, git
//! URLs, or a local folder), so the app never installs a second copy.
//!
//! Pi only folds equivalent declarations together by npm name, repository
//! URL or resolved path; the same package checked out in two folders would
//! load twice. A local folder counts as the package when its `package.json`
//! declares that name, which the caller reads and passes in.
//! A git source is matched by its repository name only: knowing the package
//! it ships would need a clone, so a repository named otherwise is missed.

/// Whether the `packages` entry `source` names the package `name`.
/// `local_name` is the `name` in the `package.json` of the folder `source`
/// points at, when it is a local folder the caller could read.
pub fn names_package(source: &str, local_name: Option<&str>, name: &str) -> bool {
    if local_name == Some(name) {
        return true;
    }
    if let Some(spec) = source.strip_prefix("npm:") {
        return npm_name(spec) == name;
    }
    if is_git(source) {
        return repo_name(source) == name;
    }
    false
}

/// The package name of an npm spec, without its version (`pkg@1.2`, `@scope/pkg@^2`).
fn npm_name(spec: &str) -> &str {
    let at = if let Some(rest) = spec.strip_prefix('@') {
        rest.find('@').map(|i| i + 1)
    } else {
        spec.find('@')
    };
    at.map_or(spec, |i| &spec[..i])
}

fn is_git(source: &str) -> bool {
    ["git:", "git+", "https://", "http://", "ssh://", "git@"]
        .iter()
        .any(|prefix| source.starts_with(prefix))
}

/// The repository's last path segment, without its `@ref` or `#ref` (refs may
/// hold slashes, as in `@release/v1`) or a `.git` suffix.
fn repo_name(source: &str) -> &str {
    let source = source.split('#').next().unwrap_or(source);
    let source = source.strip_prefix("git+").unwrap_or(source);
    let rest = ["git:", "https://", "http://", "ssh://"]
        .iter()
        .find_map(|scheme| source.strip_prefix(scheme))
        .unwrap_or(source);
    // A user (`git@host:…`, `ssh://git@host/…`) sits before the host.
    let rest = match rest.find('@') {
        Some(at) if !rest[..at].contains(['/', ':']) => &rest[at + 1..],
        _ => rest,
    };
    // Past the host, the first `@` starts the ref.
    let path_start = rest.find(['/', ':']).map_or(0, |i| i + 1);
    let path = &rest[path_start..];
    let path = path.split('@').next().unwrap_or(path);
    let last = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path);
    last.strip_suffix(".git").unwrap_or(last)
}

#[cfg(test)]
mod tests {
    use super::names_package;

    const NAME: &str = "osdy-gentle";

    #[test]
    fn a_local_folder_counts_by_the_name_its_package_declares() {
        // The checkout the user already installed by hand, under another folder name.
        assert!(names_package(
            "../../Documents/GitHub/IngracionOsdy-Pi",
            Some(NAME),
            NAME
        ));
        assert!(!names_package(
            "../../Documents/GitHub/GentleOsdy",
            Some("gentle-osdy"),
            NAME
        ));
        // A folder whose package.json couldn't be read isn't guessed from its path.
        assert!(!names_package("/tmp/osdy-gentle", None, NAME));
    }

    #[test]
    fn npm_sources_count_with_or_without_a_version() {
        assert!(names_package("npm:osdy-gentle", None, NAME));
        assert!(names_package("npm:osdy-gentle@0.1.0", None, NAME));
        assert!(!names_package("npm:osdy-gentle-extra", None, NAME));
        assert!(!names_package("npm:gentle-engram", None, NAME));
        assert!(names_package("npm:@osdy/tools@^2", None, "@osdy/tools"));
    }

    #[test]
    fn git_sources_count_by_repository_name_without_ref() {
        for source in [
            "git:github.com/OsdyOrtiz/osdy-gentle",
            "git:github.com/OsdyOrtiz/osdy-gentle@v0.1.0",
            "https://github.com/OsdyOrtiz/osdy-gentle.git",
            "https://github.com/OsdyOrtiz/osdy-gentle#main",
            "git@github.com:OsdyOrtiz/osdy-gentle.git",
            // A ref may itself hold slashes.
            "git:github.com/OsdyOrtiz/osdy-gentle@release/v1",
            "https://github.com/OsdyOrtiz/osdy-gentle.git#release/v1",
            "ssh://git@github.com/OsdyOrtiz/osdy-gentle.git",
        ] {
            assert!(names_package(source, None, NAME), "{source}");
        }
        assert!(!names_package(
            "https://github.com/OsdyOrtiz/osdy-gentle-themes.git",
            None,
            NAME
        ));
        // Another repository whose ref merely ends in the package name.
        assert!(!names_package(
            "git:github.com/OsdyOrtiz/other@release/osdy-gentle",
            None,
            NAME
        ));
    }
}
