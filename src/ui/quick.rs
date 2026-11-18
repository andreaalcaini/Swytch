//! Quick switch window: two big PC / PS5 buttons and the current input.
//!
//! It opens on the secondary display, so it stays reachable when the main one has just been
//! switched to the other input. Always on top, closed with Esc.

use std::ptr::null;

use crate::app::{app, current_label, switch_to};
use crate::i18n::{fmt, tr};
use crate::ui::{centre_in, create_frame, ctl, ensure_ready, set_text, work_area, QUICK_CLASS};
use crate::win32::*;

const ID_PC: usize = 201;
const ID_PS5: usize = 202;
const TIMER_REFRESH: usize = 1;

pub fn show() {
    let a = app();
    if a.quick != 0 {
        // SAFETY: our own window.
        unsafe { SetForegroundWindow(a.quick) };
        return;
    }
    ensure_ready();
    let t = tr();
    let wa = work_area(true);
    let ex = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_DLGMODALFRAME;
    let hwnd = create_frame(QUICK_CLASS, t.quick_title, ex, 320, 170, |w, h| centre_in(wa, w, h));
    if hwnd == 0 {
        return;
    }
    a.quick = hwnd;
    a.quick_status = ctl(hwnd, "STATIC", "", SS_CENTER | SS_CENTERIMAGE, 0, (10, 12, 300, 26), 0, a.font);
    let style = BS_MULTILINE | WS_TABSTOP;
    let pc = ctl(hwnd, "BUTTON", t.quick_pc, style, 0, (14, 50, 140, 80), ID_PC as isize, a.font_big);
    ctl(hwnd, "BUTTON", t.quick_ps5, style, 0, (166, 50, 140, 80), ID_PS5 as isize, a.font_big);
    update_status();
    // SAFETY: our own windows.
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(pc);
    }
}

fn update_status() {
    let a = app();
    if a.quick_status != 0 {
        set_text(a.quick_status, &fmt(tr().quick_current, &[&current_label()]));
    }
}

pub unsafe extern "system" fn proc_(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            match wp & 0xFFFF {
                id @ (ID_PC | ID_PS5) => {
                    let st = app().quick_status;
                    set_text(st, tr().quick_switching);
                    UpdateWindow(st);
                    switch_to(id == ID_PS5);
                    // The monitor needs a moment before it reports the new input.
                    SetTimer(h, TIMER_REFRESH, 1200, null());
                }
                IDCANCEL => {
                    DestroyWindow(h);
                }
                _ => {}
            }
            0
        }
        WM_TIMER => {
            KillTimer(h, TIMER_REFRESH);
            update_status();
            0
        }
        WM_CLOSE => {
            DestroyWindow(h);
            0
        }
        WM_DESTROY => {
            let a = app();
            a.quick = 0;
            a.quick_status = 0;
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}
