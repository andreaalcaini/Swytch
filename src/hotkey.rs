//! A global hotkey: `RegisterHotKey` modifiers plus a virtual-key code.
//!
//! In the config a hotkey is `modifiers,key` in hex (`0x03,0x7C` = Ctrl+Alt+F13). On the Italian
//! layout AltGr is Ctrl+Alt, so that combination is shown as "AltGr". In the recorder control the
//! hotkey is packed into a window's `GWLP_USERDATA` as `(modifiers << 16) | key`.

use crate::config::parse_num;
use crate::i18n::{tr, Strings};
use crate::win32::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

/// AltGr on the Italian layout (Windows reports it as Ctrl+Alt).
pub const MOD_ALTGR: u32 = MOD_CONTROL | MOD_ALT;

pub const VK_F13: u32 = 0x7C;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hotkey {
    pub m: u32,
    pub vk: u32,
}

impl Hotkey {
    pub const NONE: Hotkey = Hotkey { m: 0, vk: 0 };

    pub fn is_set(&self) -> bool {
        self.vk != 0
    }

    pub fn pack(&self) -> isize {
        ((self.m as isize) << 16) | self.vk as isize
    }

    pub fn unpack(v: isize) -> Hotkey {
        Hotkey { m: ((v >> 16) & 0xFF) as u32, vk: (v & 0xFFFF) as u32 }
    }

    pub fn label(&self) -> String {
        self.label_in(tr())
    }

    pub fn label_in(&self, s: &Strings) -> String {
        if !self.is_set() {
            return s.hotkey_none.into();
        }
        let mut p: Vec<String> = Vec::new();
        let ctrl = self.m & MOD_CONTROL != 0;
        let alt = self.m & MOD_ALT != 0;
        if ctrl && alt {
            p.push("AltGr".into());
        } else {
            if ctrl {
                p.push("Ctrl".into());
            }
            if alt {
                p.push("Alt".into());
            }
        }
        if self.m & MOD_SHIFT != 0 {
            p.push("Shift".into());
        }
        if self.m & MOD_WIN != 0 {
            p.push("Win".into());
        }
        p.push(key_name(self.vk, s));
        p.join("+")
    }

    pub fn serialize(&self) -> String {
        format!("0x{:02X},0x{:02X}", self.m, self.vk)
    }

    /// Parses `modifiers,key`; a missing or invalid part keeps the value from `fallback`.
    pub fn parse(s: &str, fallback: Hotkey) -> Hotkey {
        match s.split_once(',') {
            Some((a, b)) => Hotkey {
                m: parse_num(a).and_then(|v| u32::try_from(v).ok()).filter(|&v| v <= 0xF).unwrap_or(fallback.m),
                vk: parse_num(b).and_then(|v| u32::try_from(v).ok()).filter(|&v| v <= 0xFF).unwrap_or(fallback.vk),
            },
            None => fallback,
        }
    }
}

fn key_name(vk: u32, s: &Strings) -> String {
    let named = match vk {
        0x70..=0x87 => return format!("F{}", vk - 0x6F),
        0x30..=0x39 | 0x41..=0x5A => return char::from(vk as u8).to_string(),
        0x60..=0x69 => return format!("Num{}", vk - 0x60),
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => s.key_enter,
        0x13 => s.key_pause,
        0x1B => "Esc",
        0x20 => s.key_space,
        0x21 => s.key_pgup,
        0x22 => s.key_pgdn,
        0x23 => s.key_end,
        0x24 => "Home",
        0x25 => s.key_left,
        0x26 => s.key_up,
        0x27 => s.key_right,
        0x28 => s.key_down,
        0x2C => s.key_prtsc,
        0x2D => "Ins",
        0x2E => s.key_del,
        0x6A => "Num*",
        0x6B => "Num+",
        0x6D => "Num-",
        0x6E => "Num.",
        0x6F => "Num/",
        0x91 => s.key_scrlk,
        0xAD => s.key_mute,
        0xAE => "Vol-",
        0xAF => "Vol+",
        0xB0 => s.key_next,
        0xB1 => s.key_prev,
        0xB3 => "Play",
        _ => return format!("VK 0x{vk:02X}"),
    };
    named.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{EN, IT};

    #[test]
    fn pack_round_trip() {
        let hk = Hotkey { m: MOD_ALTGR | MOD_SHIFT, vk: VK_F13 };
        assert_eq!(Hotkey::unpack(hk.pack()), hk);
        assert_eq!(Hotkey::unpack(Hotkey::NONE.pack()), Hotkey::NONE);
    }

    #[test]
    fn serialize_round_trip() {
        let hk = Hotkey { m: MOD_ALTGR, vk: VK_F13 };
        assert_eq!(hk.serialize(), "0x03,0x7C");
        assert_eq!(Hotkey::parse(&hk.serialize(), Hotkey::NONE), hk);
    }

    #[test]
    fn parse_falls_back_per_part() {
        let fb = Hotkey { m: MOD_ALTGR, vk: VK_F13 };
        assert_eq!(Hotkey::parse("garbage", fb), fb);
        assert_eq!(Hotkey::parse("x,0x7D", fb), Hotkey { m: MOD_ALTGR, vk: 0x7D });
        assert_eq!(Hotkey::parse("0x01,", fb), Hotkey { m: MOD_ALT, vk: VK_F13 });
        assert_eq!(Hotkey::parse("0x01,0x1FF", fb), Hotkey { m: MOD_ALT, vk: VK_F13 });
        assert_eq!(Hotkey::parse("0x00,0x00", fb), Hotkey::NONE);
    }

    #[test]
    fn labels() {
        assert_eq!(Hotkey { m: MOD_ALTGR, vk: VK_F13 }.label_in(&EN), "AltGr+F13");
        assert_eq!(Hotkey { m: MOD_CONTROL | MOD_SHIFT, vk: 0x41 }.label_in(&EN), "Ctrl+Shift+A");
        assert_eq!(Hotkey { m: MOD_WIN, vk: 0x2E }.label_in(&IT), "Win+Canc");
        assert_eq!(Hotkey::NONE.label_in(&EN), "(none)");
        assert_eq!(Hotkey { m: 0, vk: 0xE9 }.label_in(&EN), "VK 0xE9");
    }
}
