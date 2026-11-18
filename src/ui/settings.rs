//! Settings window: monitor, input codes, hotkeys (with the recorder), start at sign-in.
//!
//! The global hotkeys are unregistered while it is open, otherwise Windows would swallow the
//! combination and the recorder would never see it; closing the window registers them again.

use std::ptr::null;

use crate::app::{app, apply_hotkeys, unregister_hotkeys};
use crate::autostart::{install_task, uninstall_task};
use crate::config::{parse_num, Config};
use crate::ddc;
use crate::hotkey::Hotkey;
use crate::i18n::{fmt, tr};
use crate::ui::hotkey_box;
use crate::ui::{
    centre_in, create_frame, ctl, ensure_ready, get_text, message_box, work_area, HOTKEY_BOX_CLASS, SETTINGS_CLASS,
};
use crate::win32::*;
use crate::VERSION;

const ID_COMBO: isize = 10;
const ID_DP: isize = 11;
const ID_HDMI: isize = 12;
const ID_HOTKEYS: isize = 13;
const ID_AUTOSTART: isize = 14;
const ID_BOX0: isize = 20;
const ID_DETECT: usize = 101;
const ID_DEFAULTS: usize = 102;

/// Handles of the controls whose values are read back on OK.
#[derive(Clone, Copy)]
pub struct SettingsUi {
    combo: HWND,
    dp: HWND,
    hdmi: HWND,
    chk_hotkeys: HWND,
    chk_autostart: HWND,
    boxes: [HWND; 4],
}

pub fn show() {
    let a = app();
    if a.settings != 0 {
        // SAFETY: our own window.
        unsafe { SetForegroundWindow(a.settings) };
        return;
    }
    ensure_ready();
    unregister_hotkeys();

    let t = tr();
    let wa = work_area(false);
    let title = fmt(t.settings_title, &[&VERSION]);
    let hwnd = create_frame(SETTINGS_CLASS, &title, 0, 400, 410, |w, h| centre_in(wa, w, h));
    if hwnd == 0 {
        apply_hotkeys();
        return;
    }
    a.settings = hwnd;
    let f = a.font;
    let cfg = a.cfg.clone();

    ctl(hwnd, "STATIC", t.monitor_label, 0, 0, (14, 14, 372, 20), 0, f);
    let combo_style = CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP;
    let combo = ctl(hwnd, "COMBOBOX", "", combo_style, 0, (14, 36, 372, 200), ID_COMBO, f);
    let add = |text: &str| {
        // SAFETY: NUL-terminated string for the duration of the call.
        unsafe { SendMessageW(combo, CB_ADDSTRING, 0, wide(text).as_ptr() as LPARAM) };
    };
    add(t.monitor_auto);
    let list = ddc::monitors();
    for (i, p) in list.0.iter().enumerate() {
        let note = if ddc::vcp_input(p.h).is_some() { "" } else { t.monitor_no_ddc };
        add(&format!("#{i}  {}{note}", p.name));
    }
    let count = list.0.len() as i32;
    drop(list);
    let sel = if cfg.mon >= 0 && cfg.mon < count { cfg.mon + 1 } else { 0 };

    let edit = ES_AUTOHSCROLL | WS_TABSTOP;
    ctl(hwnd, "STATIC", t.dp_code_label, 0, 0, (14, 73, 170, 20), 0, f);
    let dp = ctl(hwnd, "EDIT", &format!("0x{:02X}", cfg.dp), edit, WS_EX_CLIENTEDGE, (190, 70, 80, 23), ID_DP, f);
    ctl(hwnd, "STATIC", t.hdmi_code_label, 0, 0, (14, 101, 170, 20), 0, f);
    let hdmi = ctl(hwnd, "EDIT", &format!("0x{:02X}", cfg.hdmi), edit, WS_EX_CLIENTEDGE, (190, 98, 80, 23), ID_HDMI, f);

    ctl(hwnd, "BUTTON", t.hotkeys_group, BS_GROUPBOX, 0, (14, 134, 372, 190), 0, f);
    let check = BS_AUTOCHECKBOX | WS_TABSTOP;
    let chk_hotkeys = ctl(hwnd, "BUTTON", t.hotkeys_enable, check, 0, (26, 156, 350, 20), ID_HOTKEYS, f);
    let mut boxes = [0isize; 4];
    for (i, action) in t.actions().iter().enumerate() {
        let y = 184 + i as i32 * 28;
        ctl(hwnd, "STATIC", &format!("{action}:"), 0, 0, (26, y + 3, 130, 20), 0, f);
        let id = ID_BOX0 + i as isize;
        boxes[i] = ctl(hwnd, HOTKEY_BOX_CLASS, "", WS_TABSTOP, WS_EX_CLIENTEDGE, (160, y, 214, 23), id, f);
        hotkey_box::set(boxes[i], cfg.hk[i]);
    }
    ctl(hwnd, "STATIC", t.hotkeys_hint, 0, 0, (26, 298, 350, 20), 0, f);

    let chk_autostart = ctl(hwnd, "BUTTON", t.autostart_label, check, 0, (14, 334, 372, 20), ID_AUTOSTART, f);
    ctl(hwnd, "BUTTON", t.btn_detect, WS_TABSTOP, 0, (14, 368, 110, 28), ID_DETECT as isize, f);
    ctl(hwnd, "BUTTON", t.btn_defaults, WS_TABSTOP, 0, (130, 368, 80, 28), ID_DEFAULTS as isize, f);
    ctl(hwnd, "BUTTON", t.btn_ok, BS_DEFPUSHBUTTON | WS_TABSTOP, 0, (220, 368, 80, 28), IDOK as isize, f);
    ctl(hwnd, "BUTTON", t.btn_cancel, WS_TABSTOP, 0, (306, 368, 80, 28), IDCANCEL as isize, f);

    a.settings_ui = Some(SettingsUi { combo, dp, hdmi, chk_hotkeys, chk_autostart, boxes });
    // SAFETY: our own windows.
    unsafe {
        SendMessageW(combo, CB_SETCURSEL, sel as usize, 0);
        SendMessageW(chk_hotkeys, BM_SETCHECK, cfg.hotkeys as usize, 0);
        SendMessageW(chk_autostart, BM_SETCHECK, cfg.autostart as usize, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(combo);
    }
}

/// The first pair of actions sharing the same hotkey.
fn duplicate(hks: &[Hotkey; 4]) -> Option<(usize, usize)> {
    (0..4).flat_map(|i| (i + 1..4).map(move |j| (i, j))).find(|&(i, j)| hks[i].is_set() && hks[i] == hks[j])
}

fn on_ok(h: HWND) {
    let a = app();
    let Some(ui) = a.settings_ui else { return };
    let t = tr();
    let hks = ui.boxes.map(hotkey_box::get);
    if let Some((i, j)) = duplicate(&hks) {
        let actions = t.actions();
        let text = fmt(t.duplicate_text, &[&hks[i].label(), &actions[i], &actions[j]]);
        message_box(h, &text, t.duplicate_title, MB_ICONWARNING);
        return;
    }
    let d = Config::defaults();
    let code = |edit: HWND, fallback: u32| {
        parse_num(&get_text(edit)).and_then(|v| u32::try_from(v).ok()).filter(|&v| v <= 0xFFFF).unwrap_or(fallback)
    };
    // SAFETY: reading the state of our own controls.
    let (sel, hotkeys, autostart) = unsafe {
        (
            SendMessageW(ui.combo, CB_GETCURSEL, 0, 0) as i32,
            SendMessageW(ui.chk_hotkeys, BM_GETCHECK, 0, 0) == 1,
            SendMessageW(ui.chk_autostart, BM_GETCHECK, 0, 0) == 1,
        )
    };
    let mut c = a.cfg.clone();
    c.mon = if sel <= 0 { -1 } else { sel - 1 };
    c.dp = code(ui.dp, d.dp);
    c.hdmi = code(ui.hdmi, d.hdmi);
    c.hotkeys = hotkeys;
    c.autostart = autostart;
    c.hk = hks;
    let autostart_changed = c.autostart != a.cfg.autostart;
    a.cfg = c;
    a.cfg.save_logged();
    if autostart_changed {
        let ok = if a.cfg.autostart {
            std::env::current_exe().map(|e| install_task(&e)).unwrap_or(false)
        } else {
            uninstall_task()
        };
        if !ok {
            message_box(h, t.autostart_failed, "Swytch", MB_ICONWARNING);
        }
    }
    // SAFETY: our own window; WM_DESTROY registers the hotkeys again with the new config.
    unsafe { DestroyWindow(h) };
}

pub unsafe extern "system" fn proc_(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            match wp & 0xFFFF {
                IDOK => on_ok(h),
                IDCANCEL => {
                    DestroyWindow(h);
                }
                ID_DETECT => {
                    let t = tr();
                    message_box(h, &ddc::report(t), t.report_title, MB_ICONINFORMATION);
                }
                ID_DEFAULTS => {
                    if let Some(ui) = app().settings_ui {
                        let d = Config::defaults();
                        for (b, hk) in ui.boxes.iter().zip(d.hk) {
                            hotkey_box::set(*b, hk);
                            InvalidateRect(*b, null(), 1);
                        }
                    }
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(h);
            0
        }
        WM_DESTROY => {
            let a = app();
            a.settings = 0;
            a.settings_ui = None;
            apply_hotkeys();
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates_ignore_empty_hotkeys() {
        let a = Hotkey { m: 3, vk: 0x7C };
        let b = Hotkey { m: 3, vk: 0x7D };
        assert_eq!(duplicate(&[a, b, Hotkey::NONE, Hotkey::NONE]), None);
        assert_eq!(duplicate(&[a, b, Hotkey::NONE, b]), Some((1, 3)));
    }
}
