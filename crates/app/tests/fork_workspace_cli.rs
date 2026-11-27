//! Osdy Code Desktop runs next to the official Gentle Shell desktop app: with
//! both apps' data folders present, the fork's CLI reads only its own and
//! leaves both untouched when it rejects a command.
#![cfg(any(target_os = "macos", target_os = "linux"))]

#[test]
fn fork_reads_only_its_workspace_file_and_preserves_both_on_rejection() {
    use std::{fs, process::Command, time::SystemTime};

    let home = std::env::temp_dir().join(format!(
        "fork-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        home.clone()
    };
    let files = ["gentle-shell-gpui", "osdy-code-desktop"].map(|leaf| {
        let dir = data.join(leaf);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("workspaces.json");
        fs::write(
            &file,
            format!(r#"{{"workspaces":[{{"name":"{leaf}","root":"/tmp"}}]}}"#),
        )
        .unwrap();
        file
    });
    let snapshot = || files.each_ref().map(|file| fs::read(file).unwrap());
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_gentle-shell-gpui"))
            .env("HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("GENTLE_SHELL_GPUI_DATA_DIR")
            .args(args)
            .output()
            .unwrap()
    };
    let before = snapshot();
    let list = run(&["workspace", "list", "--json"]);
    let bad = run(&["workspace", "list", "--typo"]);
    let after = snapshot();
    fs::remove_dir_all(home).unwrap();

    assert_eq!(list.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&list.stdout),
        "[\n  {\n    \"name\": \"osdy-code-desktop\",\n    \"root\": \"/tmp\",\n    \"active\": true,\n    \"tabs\": 0\n  }\n]\n"
    );
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&bad.stderr),
        "Error: Invalid workspace command. Run ocd workspace --help.\n"
    );
    assert_eq!(before, after);
}
