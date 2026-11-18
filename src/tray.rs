//! The notification-area icon: add/remove, tooltip, warning balloons and the right-click menu.

use std::mem::{size_of, zeroed};
use std::ptr::null;

use crate::app::{self, app, CMD_DP, CMD_EXIT, CMD_HDMI, CMD_QUICK, CMD_SETTINGS, CMD_TOGGLE, WM_TRAY};
use crate::i18n::{fmt, tr};
use crate::win32::*;
use crate::VERSION;

fn nid() -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; all-zero is its documented empty state.
    let mut n: NOTIFYICONDATAW = unsafe { zeroed() };
    n.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = app().main;
    n.uID = 1;
    n
}

/// Adds the icon; false while the taskbar doesn't exist yet.
pub fn add() -> bool {
    let a = app();
    if a.tray {
        return true;
    }
    let mut n = nid();
    n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = a.icon;
    fill(&mut n.szTip, &format!("Swytch {VERSION}"));
    // SAFETY: `n` is fully initialised.
    a.tray = unsafe { Shell_NotifyIconW(NIM_ADD, &mut n) } != 0;
    a.tray
}

pub fn remove() {
    let mut n = nid();
    // SAFETY: `n` identifies our icon.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &mut n) };
}

pub fn set_tip(text: &str) {
    if !app().tray {
        return;
    }
    let mut n = nid();
    n.uFlags = NIF_TIP;
    fill(&mut n.szTip, text);
    // SAFETY: `n` identifies our icon.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &mut n) };
}

/// A warning notification from the icon (no-op while the icon isn't there).
pub fn balloon(text: &str) {
    if !app().tray {
        return;
    }
    let mut n = nid();
    n.uFlags = NIF_INFO;
    n.dwInfoFlags = NIIF_WARNING;
    fill(&mut n.szInfoTitle, "Swytch");
    fill(&mut n.szInfo, text);
    // SAFETY: `n` identifies our icon.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &mut n) };
}

pub fn show_menu() {
    let a = app();
    let cfg = a.cfg.clone();
    let main = a.main;
    let t = tr();
    let actions = t.actions();
    let item = |i: usize| {
        if cfg.hotkeys && cfg.hk[i].is_set() {
            fmt("{0}\t{1}", &[&actions[i], &cfg.hk[i].label()])
        } else {
            actions[i].to_string()
        }
    };
    // SAFETY: standard popup-menu sequence on our own window; strings outlive each call.
    unsafe {
        let m = CreatePopupMenu();
        AppendMenuW(m, 0, CMD_DP, wide(&item(0)).as_ptr());
        AppendMenuW(m, 0, CMD_HDMI, wide(&item(1)).as_ptr());
        AppendMenuW(m, 0, CMD_TOGGLE, wide(&item(2)).as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, 0, CMD_QUICK, wide(&item(3)).as_ptr());
        AppendMenuW(m, 0, CMD_SETTINGS, wide(t.menu_settings).as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, 0, CMD_EXIT, wide(t.menu_exit).as_ptr());
        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        // Required so the menu closes when the user clicks elsewhere.
        SetForegroundWindow(main);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, main, null());
        PostMessageW(main, WM_NULL, 0, 0);
        DestroyMenu(m);
        app::command(cmd as usize);
    }
}
