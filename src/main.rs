//! Swytch: switches the monitor input (DisplayPort ↔ HDMI) over DDC/CI, from global hotkeys, a
//! tray icon or a quick switch window. Win32 only, no dependencies. See `app` for the startup path.
//!
//! Command line (exit code 0 = success):
//!
//! | Arguments | Effect |
//! | --- | --- |
//! | *(none)* | start the tray app |
//! | `dp`, `hdmi`, `toggle` | switch the input and exit |
//! | `install` / `uninstall` | install to `%LOCALAPPDATA%\Programs\Swytch` + start at sign-in / remove the start at sign-in |
//! | `export-icon <file.ico>` | write the app icon (dev, used by `make icon`) |
//! | `icon-preview <file.bmp>` | write a preview of the icon at several sizes (dev) |

#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod config;
mod ddc;
mod hotkey;
mod i18n;
mod logging;
mod paths;
mod tray;
mod trayicon;
mod ui;
mod win32;

use std::fs;

use config::Config;
use logging::log;

/// From `Cargo.toml`, the only place the version is written (see `make version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn write_file(path: &str, bytes: &[u8]) -> i32 {
    match fs::write(path, bytes) {
        Ok(()) => 0,
        Err(e) => {
            log(&format!("Writing {path} failed: {e}"));
            1
        }
    }
}

fn cli(args: &[String]) -> i32 {
    let arg = args[0].to_ascii_lowercase();
    let out = |default: &str| args.get(1).cloned().unwrap_or_else(|| default.to_string());
    let switch = |code: u32, mon: i32| match ddc::set_input(mon, code) {
        Ok(_) => 0,
        Err(e) => {
            log(&format!("Command line: switch to 0x{code:02X} failed: {e:?}"));
            1
        }
    };
    match arg.as_str() {
        "dp" | "displayport" | "pc" => {
            let c = Config::load().0;
            switch(c.dp, c.mon)
        }
        "hdmi" | "ps5" => {
            let c = Config::load().0;
            switch(c.hdmi, c.mon)
        }
        "toggle" => {
            let c = Config::load().0;
            let target = if ddc::get_input(c.mon) == Some(c.dp) { c.hdmi } else { c.dp };
            switch(target, c.mon)
        }
        "install" => !autostart::install() as i32,
        "uninstall" => !autostart::uninstall_task() as i32,
        "export-icon" => write_file(&out("icon.ico"), &trayicon::ico_bytes(&trayicon::ICO_SIZES)),
        "icon-preview" => write_file(&out("icon-preview.bmp"), &trayicon::preview_bmp()),
        _ => 2,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        app::run();
    } else {
        std::process::exit(cli(&args));
    }
}
