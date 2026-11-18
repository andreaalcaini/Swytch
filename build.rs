//! Embeds the Windows resources into Swytch.exe: the version info (from `Cargo.toml`, shown in the
//! file's Properties and in Task Manager) and the icon (`build/windows/icon.ico`, shown by Explorer).
//!
//! Compiled with `windres` (MinGW binutils; `WINDRES` overrides the path). Without it the app still
//! builds, just without resources, unless `SWYTCH_REQUIRE_RESOURCES=1` (set in CI and releases).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build/windows/icon.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    println!("cargo:rerun-if-env-changed=SWYTCH_REQUIRE_RESOURCES");

    let required = env::var("SWYTCH_REQUIRE_RESOURCES").is_ok_and(|v| v == "1");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let rc = out.join("swytch.rc");
    let obj = out.join("swytch-res.o");
    fs::write(&rc, resource_script(&root.join("build").join("windows").join("icon.ico"))).expect("write swytch.rc");

    let windres = env::var("WINDRES").unwrap_or_else(|_| "windres".into());
    let status = Command::new(&windres)
        .arg("--input-format=rc")
        .arg("--output-format=coff")
        .arg("-i")
        .arg(&rc)
        .arg("-o")
        .arg(&obj)
        .status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        other => {
            let why = match other {
                Ok(s) => format!("{windres} failed ({s})"),
                Err(e) => format!("{windres} not found ({e})"),
            };
            if required {
                panic!("cannot embed the Windows resources: {why}");
            }
            println!("cargo:warning=Swytch.exe built without version info and icon: {why}");
        }
    }
}

fn resource_script(icon: &Path) -> String {
    let version = env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let mut parts = version.split(['.', '-', '+']).map(|p| p.parse::<u16>().unwrap_or(0));
    let (major, minor, patch) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    let icon_line = if icon.exists() {
        // windres wants forward slashes (or doubled backslashes) in quoted paths.
        format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "/"))
    } else {
        println!("cargo:warning=build/windows/icon.ico is missing: run `make icon`");
        String::new()
    };
    format!(
        r#"{icon_line}
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3F
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "andreaalcaini"
      VALUE "FileDescription", "Swytch"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "Swytch"
      VALUE "LegalCopyright", "(c) 2026 andreaalcaini, MIT License"
      VALUE "OriginalFilename", "Swytch.exe"
      VALUE "ProductName", "Swytch"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}
