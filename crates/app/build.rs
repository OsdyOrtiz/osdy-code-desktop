//! Windows: embeds the app icon (resource id 1, which GPUI loads for its
//! window class) and version info into the exe. macOS: embeds a minimal
//! Info.plist so a bare binary (`cargo run`, `target/release`) shows
//! "Osdy Code" in the menu bar instead of the executable name; inside the
//! .app bundle, the bundle's own Info.plist wins.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // GPUI constructs and lays out large element trees on the native UI
    // thread. Unoptimized Rust frames can exceed the MSVC default 1 MiB
    // executable stack, especially when an agent is first rendered.
    // Reserve virtual address space, not 8 MiB of committed memory.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bin=gentle-shell-gpui=/STACK:8388608");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        macos_info_plist();
    }
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            windows_resources();
        }
    }
}

/// Only the display names: no bundle identifier, so a development binary
/// never shares the installed app's identity (permissions, notifications).
fn macos_info_plist() {
    let out = std::env::var("OUT_DIR").expect("OUT_DIR is not set");
    let path = std::path::Path::new(&out).join("Info.plist");
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>Osdy Code</string>
	<key>CFBundleDisplayName</key>
	<string>Osdy Code</string>
</dict>
</plist>
"#;
    std::fs::write(&path, plist).expect("writing the embedded Info.plist");
    println!(
        "cargo:rustc-link-arg-bin=gentle-shell-gpui=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        path.display()
    );
}

#[cfg(windows)]
fn windows_resources() {
    use std::path::PathBuf;

    let env = |key: &str| std::env::var(key).unwrap_or_else(|_| panic!("{key} is not set"));
    let icon = PathBuf::from(env("CARGO_MANIFEST_DIR")).join("../../packaging/icons/icon.ico");
    let icon = icon
        .canonicalize()
        .expect("packaging/icons/icon.ico is missing");
    println!("cargo:rerun-if-changed={}", icon.display());

    let version = env("CARGO_PKG_VERSION");
    let numeric = format!(
        "{},{},{},0",
        env("CARGO_PKG_VERSION_MAJOR"),
        env("CARGO_PKG_VERSION_MINOR"),
        env("CARGO_PKG_VERSION_PATCH")
    );
    // `canonicalize` yields a `\\?\` verbatim path, which rc.exe rejects.
    let icon = icon.display().to_string();
    let icon = icon
        .strip_prefix(r"\\?\")
        .unwrap_or(&icon)
        .replace('\\', "\\\\");
    // Numeric constants instead of <winver.h> (VOS_NT_WINDOWS32, VFT_APP), so
    // rc.exe needs no SDK include path.
    let rc = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "OsdyOrtiz"
      VALUE "FileDescription", "Osdy Code"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "gentle-shell-gpui"
      VALUE "OriginalFilename", "gentle-shell-gpui.exe"
      VALUE "ProductName", "Osdy Code"
      VALUE "ProductVersion", "{version}"
      VALUE "LegalCopyright", "MIT License"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let path = PathBuf::from(env("OUT_DIR")).join("gentle-shell-gpui.rc");
    std::fs::write(&path, rc).expect("writing the resource script");
    embed_resource::compile(&path, embed_resource::NONE)
        .manifest_optional()
        .expect("compiling Windows resources");
}
