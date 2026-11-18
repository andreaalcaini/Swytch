//! Hotkey recorder: a focusable box that captures the next key combination pressed in it.
//!
//! The hotkey is stored in the window's `GWLP_USERDATA` (see [`Hotkey::pack`]), plus [`REC_FLAG`]
//! once a combination has been pressed since the box got focus: until then a focused box shows
//! the "press the combination" prompt. Tab/Shift+Tab move focus, Esc cancels the dialog,
//! Del/Backspace clear the hotkey, Enter is ignored.

use std::mem::zeroed;
use std::ptr::null;

use crate::app::{app, s};
use crate::hotkey::Hotkey;
use crate::i18n::tr;
use crate::win32::*;

const REC_FLAG: isize = 1 << 24;

const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_DELETE: u32 = 0x2E;

pub fn set(h: HWND, hk: Hotkey) {
    // SAFETY: our own control.
    unsafe { SetWindowLongPtrW(h, GWLP_USERDATA, hk.pack()) };
}

pub fn get(h: HWND) -> Hotkey {
    // SAFETY: our own control.
    Hotkey::unpack(unsafe { GetWindowLongPtrW(h, GWLP_USERDATA) })
}

fn record(h: HWND, vk: u32) {
    // Modifiers alone: wait for the real key.
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5) {
        return;
    }
    // SAFETY: key-state queries and messages to our own windows.
    unsafe {
        let down = |k: i32| GetKeyState(k) < 0;
        let mut m = 0u32;
        if down(0x11) {
            m |= MOD_CONTROL;
        }
        if down(0x12) {
            m |= MOD_ALT;
        }
        if down(0x10) {
            m |= MOD_SHIFT;
        }
        if down(0x5B) || down(0x5C) {
            m |= MOD_WIN;
        }

        let parent = GetParent(h);
        if vk == VK_TAB && (m == 0 || m == MOD_SHIFT) {
            SetFocus(GetNextDlgTabItem(parent, h, (m == MOD_SHIFT) as BOOL));
            return;
        }
        let hk = if m == 0 {
            match vk {
                VK_ESCAPE => {
                    PostMessageW(parent, WM_COMMAND, IDCANCEL, 0);
                    return;
                }
                VK_RETURN => return,
                VK_DELETE | VK_BACK => Hotkey::NONE,
                _ => Hotkey { m, vk },
            }
        } else {
            Hotkey { m, vk }
        };
        SetWindowLongPtrW(h, GWLP_USERDATA, hk.pack() | REC_FLAG);
        InvalidateRect(h, null(), 1);
    }
}

pub unsafe extern "system" fn proc_(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_GETDLGCODE => DLGC_WANTALLKEYS,
        WM_LBUTTONDOWN => {
            SetFocus(h);
            0
        }
        WM_SETFOCUS => {
            let v = GetWindowLongPtrW(h, GWLP_USERDATA) & !REC_FLAG;
            SetWindowLongPtrW(h, GWLP_USERDATA, v);
            InvalidateRect(h, null(), 1);
            0
        }
        WM_KILLFOCUS => {
            InvalidateRect(h, null(), 1);
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            record(h, wp as u32);
            0
        }
        // Nothing reaches DefWindowProc: no system menu on Alt/F10, no beep.
        WM_CHAR | WM_SYSCHAR | WM_KEYUP | WM_SYSKEYUP => 0,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(h, &mut ps);
            let mut rc = RECT::default();
            GetClientRect(h, &mut rc);
            let focused = GetFocus() == h;
            let v = GetWindowLongPtrW(h, GWLP_USERDATA);
            if focused {
                let br = CreateSolidBrush(0x00D7_FAFF); // RGB(255,250,215) as 0x00BBGGRR
                FillRect(dc, &rc, br);
                DeleteObject(br);
            } else {
                FillRect(dc, &rc, GetSysColorBrush(5)); // COLOR_WINDOW
            }
            let old = SelectObject(dc, app().font);
            SetBkMode(dc, 1); // TRANSPARENT
            SetTextColor(dc, GetSysColor(8)); // COLOR_WINDOWTEXT
            let text =
                if focused && v & REC_FLAG == 0 { tr().hotkey_press.to_string() } else { Hotkey::unpack(v).label() };
            rc.left += s(6);
            DrawTextW(dc, wide(&text).as_ptr(), -1, &mut rc, 0x20 | 0x4 | 0x800); // SINGLELINE|VCENTER|NOPREFIX
            SelectObject(dc, old);
            EndPaint(h, &ps);
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}
