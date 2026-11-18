//! Shared pieces of the two windows (quick switch and settings), created lazily on first use.
//!
//! Visual styles (Common Controls v6) come from an activation context built from a small manifest
//! written to the data folder, instead of a manifest embedded in the exe: the tray-only startup
//! never loads comctl32 v6, fonts or window classes.

pub mod hotkey_box;
pub mod quick;
pub mod settings;

use std::fs;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use crate::app::{app, s};
use crate::ddc::hmonitors;
use crate::logging::log;
use crate::paths::data_dir;
use crate::trayicon::make_icon;
use crate::win32::*;

pub const QUICK_CLASS: &str = "msQuick";
pub const SETTINGS_CLASS: &str = "msConfig";
pub const HOTKEY_BOX_CLASS: &str = "msHotkey";

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>
"#;

type WndProcFn = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Visual styles, fonts and window classes; done once, the first time a window opens.
pub fn ensure_ready() {
    let a = app();
    if a.ui_ready {
        return;
    }
    let path = data_dir().join("visualstyles.manifest");
    if fs::read_to_string(&path).map(|t| t != MANIFEST).unwrap_or(true) {
        if let Err(e) = fs::create_dir_all(data_dir()).and_then(|_| fs::write(&path, MANIFEST)) {
            log(&format!("UI: writing {path:?} failed, classic controls: {e}"));
        }
    }
    // SAFETY: Win32 setup on the UI thread; every pointer passed lives for the duration of its call.
    unsafe {
        let wpath = wide(&path.to_string_lossy());
        let mut ctx: ACTCTXW = zeroed();
        ctx.cbSize = size_of::<ACTCTXW>() as u32;
        ctx.lpSource = wpath.as_ptr();
        let h = CreateActCtxW(&ctx);
        if h != -1 {
            let mut cookie = 0usize;
            ActivateActCtx(h, &mut cookie);
        }

        let face = wide("Segoe UI");
        a.font = CreateFontW(-(9 * a.dpi as i32 / 72), 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
        a.font_big = CreateFontW(-(11 * a.dpi as i32 / 72), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());

        let arrow = LoadCursorW(0, 32512 as *const u16); // IDC_ARROW
        let hand = LoadCursorW(0, 32649 as *const u16); // IDC_HAND
        let big = make_icon(GetSystemMetrics(SM_CXICON)); // taskbar / Alt+Tab
        let classes: [(&str, WndProcFn, isize, isize); 3] = [
            (QUICK_CLASS, quick::proc_, arrow, 16), // COLOR_BTNFACE + 1
            (SETTINGS_CLASS, settings::proc_, arrow, 16),
            (HOTKEY_BOX_CLASS, hotkey_box::proc_, hand, 0),
        ];
        for (name, proc_, cursor, bg) in classes {
            let cls = wide(name);
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(proc_),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: a.hinst,
                hIcon: big,
                hCursor: cursor,
                hbrBackground: bg,
                lpszMenuName: null(),
                lpszClassName: cls.as_ptr(),
                hIconSm: a.icon,
            };
            RegisterClassExW(&wc);
        }
    }
    a.ui_ready = true;
}

/// A child control at 96-DPI coordinates (scaled to the system DPI).
#[allow(clippy::too_many_arguments)]
pub fn ctl(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    ex: u32,
    (x, y, w, h): (i32, i32, i32, i32),
    id: isize,
    font: isize,
) -> HWND {
    // SAFETY: valid parent and NUL-terminated strings for the duration of the call.
    unsafe {
        let hw = CreateWindowExW(
            ex,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            s(x),
            s(y),
            s(w),
            s(h),
            parent,
            id,
            app().hinst,
            null_mut(),
        );
        SendMessageW(hw, WM_SETFONT, font as usize, 1);
        hw
    }
}

pub fn get_text(h: HWND) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: the buffer length is passed along.
    let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn set_text(h: HWND, text: &str) {
    // SAFETY: NUL-terminated string for the duration of the call.
    unsafe { SetWindowTextW(h, wide(text).as_ptr()) };
}

pub fn message_box(owner: HWND, text: &str, caption: &str, flags: u32) {
    // SAFETY: NUL-terminated strings for the duration of the call.
    unsafe { MessageBoxW(owner, wide(text).as_ptr(), wide(caption).as_ptr(), flags) };
}

/// A captioned top-level window whose client area is cw×ch (96-DPI units), placed by `place(w, h)`.
pub fn create_frame(
    class: &str,
    title: &str,
    ex: u32,
    cw: i32,
    ch: i32,
    place: impl Fn(i32, i32) -> (i32, i32),
) -> HWND {
    let style = WS_CAPTION | WS_SYSMENU;
    let mut rc = RECT { left: 0, top: 0, right: s(cw), bottom: s(ch) };
    // SAFETY: valid RECT and NUL-terminated strings.
    unsafe {
        AdjustWindowRectEx(&mut rc, style, 0, ex);
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let (x, y) = place(w, h);
        CreateWindowExW(
            ex,
            wide(class).as_ptr(),
            wide(title).as_ptr(),
            style,
            x,
            y,
            w,
            h,
            0,
            0,
            app().hinst,
            null_mut(),
        )
    }
}

/// Centres a w×h window in `area`.
pub fn centre_in(area: RECT, w: i32, h: i32) -> (i32, i32) {
    (area.left + (area.right - area.left - w) / 2, area.top + (area.bottom - area.top - h) / 2)
}

/// Work area of the first non-primary display (or of the primary if it is the only one).
pub fn work_area(secondary: bool) -> RECT {
    let mut primary = RECT::default();
    let mut other: Option<RECT> = None;
    for hm in hmonitors() {
        // SAFETY: zeroed struct with its size set, as the API requires.
        let mut mi: MONITORINFOEXW = unsafe { zeroed() };
        mi.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(hm, &mut mi) } == 0 {
            continue;
        }
        if mi.dwFlags & 1 != 0 {
            primary = mi.rcWork; // MONITORINFOF_PRIMARY
        } else if other.is_none() {
            other = Some(mi.rcWork);
        }
    }
    if secondary {
        other.unwrap_or(primary)
    } else {
        primary
    }
}
