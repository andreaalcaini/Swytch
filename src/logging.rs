//! Plain-text log in `%APPDATA%\Swytch\Swytch.log`, rotated to `Swytch.log.1` past 1 MiB.
//!
//! The app logs only a handful of lines per session (startup timing, hotkeys, switches), so the file
//! is opened per line: nothing stays open and nothing is lost if the process is killed.

use std::fs;
use std::io::Write;

use crate::paths::data_dir;
use crate::win32::{GetCurrentProcess, GetLocalTime, GetProcessTimes, GetSystemTimeAsFileTime, SYSTEMTIME};

const MAX_BYTES: u64 = 1024 * 1024;

pub fn log(msg: &str) {
    let mut t = SYSTEMTIME::default();
    // SAFETY: `t` is a valid out pointer.
    unsafe { GetLocalTime(&mut t) };
    let dir = data_dir();
    if fs::create_dir_all(&dir).is_err() {
        return; // nowhere to log to: logging must never take the app down
    }
    let path = dir.join("Swytch.log");
    if fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("Swytch.log.1"));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}  {}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds, msg
        );
    }
}

/// Milliseconds since this process was created (includes the loader's work before `main`).
pub fn ms_since_process_start() -> u64 {
    let (mut created, mut exited, mut kernel, mut user, mut now) = (0u64, 0u64, 0u64, 0u64, 0u64);
    // SAFETY: all out pointers are valid FILETIME-sized integers.
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user);
        GetSystemTimeAsFileTime(&mut now);
    }
    now.saturating_sub(created) / 10_000
}
