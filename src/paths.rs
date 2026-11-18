//! Where Swytch keeps its files.
//!
//! Folders come from the shell (`SHGetKnownFolderPath`), not from environment variables: a process
//! started by the Task Scheduler may run without `APPDATA`/`LOCALAPPDATA`.

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::OnceLock;

use crate::win32::{from_wide, CoTaskMemFree, GetUserNameW, SHGetKnownFolderPath, GUID};

pub const APP_NAME: &str = "Swytch";
/// Name of the previous version: its config is migrated and its start at login removed.
pub const LEGACY_NAME: &str = "monitorSwitch";

// FOLDERID_RoamingAppData {3EB685DB-65F9-4CF6-A03A-E3EF65729F3D}
const FOLDERID_ROAMING_APP_DATA: GUID =
    GUID { d1: 0x3EB6_85DB, d2: 0x65F9, d3: 0x4CF6, d4: [0xA0, 0x3A, 0xE3, 0xEF, 0x65, 0x72, 0x9F, 0x3D] };
// FOLDERID_LocalAppData {F1B32785-6FBA-4FCF-9D55-7B8E7F157091}
const FOLDERID_LOCAL_APP_DATA: GUID =
    GUID { d1: 0xF1B3_2785, d2: 0x6FBA, d3: 0x4FCF, d4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91] };

fn known_folder(id: &GUID, env_fallback: &str) -> PathBuf {
    let mut p: *mut u16 = null_mut();
    // SAFETY: on success the shell returns a NUL-terminated string that we must free with CoTaskMemFree.
    let base = unsafe {
        let ok = SHGetKnownFolderPath(id, 0, 0, &mut p) == 0 && !p.is_null();
        let r = if ok {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            Some(PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(p, n))))
        } else {
            None
        };
        if !p.is_null() {
            CoTaskMemFree(p as *mut c_void);
        }
        r
    };
    base.or_else(|| std::env::var_os(env_fallback).map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("."))
}

/// `%APPDATA%` (Roaming).
pub fn roaming() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| known_folder(&FOLDERID_ROAMING_APP_DATA, "APPDATA")).clone()
}

/// `%APPDATA%\Swytch`: `config.ini`, `Swytch.log`.
pub fn data_dir() -> PathBuf {
    roaming().join(APP_NAME)
}

/// `%APPDATA%\monitorSwitch`: data folder of the previous version.
pub fn legacy_data_dir() -> PathBuf {
    roaming().join(LEGACY_NAME)
}

/// `%LOCALAPPDATA%\Programs\Swytch`: where `Swytch.exe install` copies the executable.
pub fn install_dir() -> PathBuf {
    known_folder(&FOLDERID_LOCAL_APP_DATA, "LOCALAPPDATA").join("Programs").join(APP_NAME)
}

/// Name of the signed-in user (the scheduled task runs as this user).
pub fn user_name() -> String {
    let mut buf = [0u16; 257];
    let mut len = buf.len() as u32;
    // SAFETY: the buffer and its length are valid for the call.
    if unsafe { GetUserNameW(buf.as_mut_ptr(), &mut len) } != 0 {
        from_wide(&buf)
    } else {
        std::env::var("USERNAME").unwrap_or_default()
    }
}
