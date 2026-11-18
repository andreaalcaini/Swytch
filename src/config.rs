//! User preferences in `%APPDATA%\Swytch\config.ini`.
//!
//! The file is durable user data: key names stay stable, and a missing or invalid value falls back
//! to its default instead of failing. Keys:
//!
//! | Key | Default | Meaning |
//! | --- | --- | --- |
//! | `MonitorIndex` | `-1` | physical monitor to drive; `-1` = first one answering VCP 0x60 |
//! | `DpCode` / `HdmiCode` | `0x0F` / `0x11` | VCP 0x60 values of the two inputs |
//! | `AutoStart` | `1` | start at sign-in (scheduled task) |
//! | `HotkeysEnabled` | `1` | register the global hotkeys |
//! | `HotkeyDp`, `HotkeyHdmi`, `HotkeyToggle`, `HotkeyWindow` | AltGr+F13…F16 | `modifiers,key` |
//! | `Language` | `auto` | `auto` (Windows language), `en`, `it` |
//!
//! Saving writes a temporary file and renames it over the old one, so a crash never leaves half a file.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::hotkey::{Hotkey, MOD_ALTGR, VK_F13};
use crate::logging::log;
use crate::paths::{data_dir, legacy_data_dir};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mon: i32,
    pub dp: u32,
    pub hdmi: u32,
    pub autostart: bool,
    pub hotkeys: bool,
    pub hk: [Hotkey; 4],
    pub language: String,
}

/// Decimal or `0x` hex integer.
pub fn parse_num(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn parse_code(v: &str, fallback: u32) -> u32 {
    parse_num(v).and_then(|x| u32::try_from(x).ok()).filter(|&x| x <= 0xFFFF).unwrap_or(fallback)
}

impl Config {
    pub fn defaults() -> Config {
        Config {
            mon: -1,
            dp: 0x0F,
            hdmi: 0x11,
            autostart: true,
            hotkeys: true,
            hk: [
                Hotkey { m: MOD_ALTGR, vk: VK_F13 },     // F13
                Hotkey { m: MOD_ALTGR, vk: VK_F13 + 1 }, // F14
                Hotkey { m: MOD_ALTGR, vk: VK_F13 + 2 }, // F15
                Hotkey { m: MOD_ALTGR, vk: VK_F13 + 3 }, // F16
            ],
            language: "auto".into(),
        }
    }

    pub fn path() -> PathBuf {
        data_dir().join("config.ini")
    }

    pub fn parse(text: &str) -> Config {
        let mut c = Config::defaults();
        let d = Config::defaults();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            let flag = v == "1" || v.eq_ignore_ascii_case("true");
            match k.trim() {
                "MonitorIndex" => {
                    c.mon = parse_num(v).and_then(|x| i32::try_from(x).ok()).filter(|&x| x >= -1).unwrap_or(-1)
                }
                "DpCode" => c.dp = parse_code(v, d.dp),
                "HdmiCode" => c.hdmi = parse_code(v, d.hdmi),
                "AutoStart" => c.autostart = flag,
                "HotkeysEnabled" => c.hotkeys = flag,
                "HotkeyDp" => c.hk[0] = Hotkey::parse(v, d.hk[0]),
                "HotkeyHdmi" => c.hk[1] = Hotkey::parse(v, d.hk[1]),
                "HotkeyToggle" => c.hk[2] = Hotkey::parse(v, d.hk[2]),
                "HotkeyWindow" => c.hk[3] = Hotkey::parse(v, d.hk[3]),
                "Language" => c.language = v.to_ascii_lowercase(),
                _ => {}
            }
        }
        c
    }

    pub fn to_ini(&self) -> String {
        format!(
            "MonitorIndex={}\r\nDpCode=0x{:02X}\r\nHdmiCode=0x{:02X}\r\nAutoStart={}\r\nHotkeysEnabled={}\r\n\
             ; hotkey = modifiers,key  (0x03 = AltGr/Ctrl+Alt, 0x7C = F13)\r\n\
             HotkeyDp={}\r\nHotkeyHdmi={}\r\nHotkeyToggle={}\r\nHotkeyWindow={}\r\n\
             ; auto, en, it\r\nLanguage={}\r\n",
            self.mon,
            self.dp,
            self.hdmi,
            self.autostart as u8,
            self.hotkeys as u8,
            self.hk[0].serialize(),
            self.hk[1].serialize(),
            self.hk[2].serialize(),
            self.hk[3].serialize(),
            self.language,
        )
    }

    /// Loads the config (migrating the previous version's file first). The flag says whether a
    /// file existed: on first run the caller writes the defaults.
    pub fn load() -> (Config, bool) {
        let path = Config::path();
        if !path.exists() {
            let old = legacy_data_dir().join("config.ini");
            if old.exists() {
                let copied = fs::create_dir_all(data_dir()).and_then(|_| fs::copy(&old, &path));
                if let Err(e) = copied {
                    log(&format!("Config: migrating {old:?} failed: {e}"));
                }
            }
        }
        match fs::read_to_string(&path) {
            Ok(text) => (Config::parse(&text), true),
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    log(&format!("Config: reading {path:?} failed, using defaults: {e}"));
                }
                (Config::defaults(), false)
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        fs::create_dir_all(data_dir())?;
        let path = Config::path();
        let tmp = path.with_extension("ini.tmp");
        fs::write(&tmp, self.to_ini())?;
        fs::rename(&tmp, &path)
    }

    /// [`save`](Self::save), logging a failure (the app keeps running with the values in memory).
    pub fn save_logged(&self) {
        if let Err(e) = self.save() {
            log(&format!("Config: saving {:?} failed: {e}", Config::path()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win32::MOD_SHIFT;

    #[test]
    fn round_trip() {
        let mut c = Config::defaults();
        c.mon = 0;
        c.dp = 0x10;
        c.hdmi = 0x12;
        c.autostart = false;
        c.hk[2] = Hotkey::NONE;
        c.hk[3] = Hotkey { m: MOD_SHIFT, vk: 0x41 };
        c.language = "it".into();
        assert_eq!(Config::parse(&c.to_ini()), c);
    }

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::parse(""), Config::defaults());
    }

    #[test]
    fn invalid_values_fall_back() {
        let c = Config::parse("MonitorIndex=-7\nDpCode=0x1FFFF\nHdmiCode=abc\nHotkeyDp=nope\nUnknown=1\n");
        let d = Config::defaults();
        assert_eq!(c.mon, -1);
        assert_eq!(c.dp, d.dp);
        assert_eq!(c.hdmi, d.hdmi);
        assert_eq!(c.hk[0], d.hk[0]);
    }

    #[test]
    fn reads_the_previous_version_format() {
        // Written by monitorSwitch (C#): no Language key, LF or CRLF line ends.
        let old = "MonitorIndex=0\r\nDpCode=0x0F\r\nHdmiCode=0x11\r\nAutoStart=1\r\nHotkeysEnabled=1\r\n\
                   HotkeyDp=0x03,0x7C\r\nHotkeyHdmi=0x03,0x7D\r\nHotkeyToggle=0x00,0x00\r\nHotkeyWindow=0x00,0x00\r\n";
        let c = Config::parse(old);
        assert_eq!(c.mon, 0);
        assert_eq!(c.hk[1], Hotkey { m: MOD_ALTGR, vk: 0x7D });
        assert_eq!(c.hk[2], Hotkey::NONE);
        assert_eq!(c.language, "auto");
    }
}
