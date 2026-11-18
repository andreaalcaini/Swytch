//! The Win32 surface the app uses: types, structs, functions and constants.
//!
//! Functions are linked with `raw-dylib`, so no import libraries (and no Windows SDK) are needed:
//! the GNU toolchain alone builds the app. Only what the app calls is declared here; every other
//! module goes through this one instead of declaring its own FFI.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;

pub type HWND = isize;
pub type BOOL = i32;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;
pub type MONITORENUMPROC = Option<unsafe extern "system" fn(isize, isize, *mut RECT, LPARAM) -> BOOL>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
    pub lPrivate: u32,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: isize,
    pub hIcon: isize,
    pub hCursor: isize,
    pub hbrBackground: isize,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: isize,
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: u32,
    pub hWnd: HWND,
    pub uID: u32,
    pub uFlags: u32,
    pub uCallbackMessage: u32,
    pub hIcon: isize,
    pub szTip: [u16; 128],
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szInfo: [u16; 256],
    pub uVersion: u32,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: u32,
    pub guidItem: [u32; 4],
    pub hBalloonIcon: isize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PHYSICAL_MONITOR {
    pub hPhysicalMonitor: isize,
    pub szPhysicalMonitorDescription: [u16; 128],
}

#[repr(C)]
pub struct MONITORINFOEXW {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
    pub szDevice: [u16; 32],
}

#[repr(C)]
pub struct DISPLAY_DEVICEW {
    pub cb: u32,
    pub DeviceName: [u16; 32],
    pub DeviceString: [u16; 128],
    pub StateFlags: u32,
    pub DeviceID: [u16; 128],
    pub DeviceKey: [u16; 128],
}

#[repr(C)]
pub struct ICONINFO {
    pub fIcon: BOOL,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: isize,
    pub hbmColor: isize,
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: isize,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
pub struct ACTCTXW {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub lpSource: *const u16,
    pub wProcessorArchitecture: u16,
    pub wLangId: u16,
    pub lpAssemblyDirectory: *const u16,
    pub lpResourceName: *const u16,
    pub lpApplicationName: *const u16,
    pub hModule: isize,
}

#[repr(C)]
#[derive(Default)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

#[repr(C)]
pub struct GUID {
    pub d1: u32,
    pub d2: u16,
    pub d3: u16,
    pub d4: [u8; 8],
}

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn RegisterClassExW(wc: *const WNDCLASSEXW) -> u16;
    #[allow(clippy::too_many_arguments)]
    pub fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: isize,
        inst: isize,
        param: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    pub fn GetMessageW(msg: *mut MSG, h: HWND, a: u32, b: u32) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    pub fn IsDialogMessageW(h: HWND, msg: *mut MSG) -> BOOL;
    pub fn PostQuitMessage(code: i32);
    pub fn RegisterHotKey(h: HWND, id: i32, mods: u32, vk: u32) -> BOOL;
    pub fn UnregisterHotKey(h: HWND, id: i32) -> BOOL;
    pub fn RegisterWindowMessageW(name: *const u16) -> u32;
    pub fn SetTimer(h: HWND, id: usize, ms: u32, f: *const c_void) -> usize;
    pub fn KillTimer(h: HWND, id: usize) -> BOOL;
    pub fn CreatePopupMenu() -> isize;
    pub fn AppendMenuW(m: isize, flags: u32, id: usize, text: *const u16) -> BOOL;
    pub fn TrackPopupMenu(m: isize, flags: u32, x: i32, y: i32, r: i32, h: HWND, rc: *const RECT) -> BOOL;
    pub fn DestroyMenu(m: isize) -> BOOL;
    pub fn SetForegroundWindow(h: HWND) -> BOOL;
    pub fn GetCursorPos(p: *mut POINT) -> BOOL;
    pub fn PostMessageW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> BOOL;
    pub fn SendMessageW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    pub fn DestroyWindow(h: HWND) -> BOOL;
    pub fn ShowWindow(h: HWND, cmd: i32) -> BOOL;
    pub fn FindWindowW(class: *const u16, title: *const u16) -> HWND;
    pub fn UpdateWindow(h: HWND) -> BOOL;
    pub fn SetWindowTextW(h: HWND, s: *const u16) -> BOOL;
    pub fn GetWindowTextW(h: HWND, s: *mut u16, n: i32) -> i32;
    pub fn MessageBoxW(h: HWND, text: *const u16, cap: *const u16, t: u32) -> i32;
    pub fn GetSystemMetrics(i: i32) -> i32;
    pub fn CreateIconIndirect(ii: *const ICONINFO) -> isize;
    pub fn EnumDisplayMonitors(dc: isize, clip: *const RECT, f: MONITORENUMPROC, l: LPARAM) -> BOOL;
    pub fn GetMonitorInfoW(m: isize, mi: *mut MONITORINFOEXW) -> BOOL;
    pub fn EnumDisplayDevicesW(dev: *const u16, n: u32, dd: *mut DISPLAY_DEVICEW, flags: u32) -> BOOL;
    pub fn SetProcessDpiAwarenessContext(ctx: isize) -> BOOL;
    pub fn GetDpiForSystem() -> u32;
    pub fn GetKeyState(vk: i32) -> i16;
    pub fn InvalidateRect(h: HWND, rc: *const RECT, erase: BOOL) -> BOOL;
    pub fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> isize;
    pub fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(dc: isize, rc: *const RECT, br: isize) -> i32;
    pub fn GetClientRect(h: HWND, rc: *mut RECT) -> BOOL;
    pub fn SetFocus(h: HWND) -> HWND;
    pub fn GetFocus() -> HWND;
    pub fn GetParent(h: HWND) -> HWND;
    pub fn GetNextDlgTabItem(dlg: HWND, ctl: HWND, prev: BOOL) -> HWND;
    pub fn SetWindowLongPtrW(h: HWND, i: i32, v: isize) -> isize;
    pub fn GetWindowLongPtrW(h: HWND, i: i32) -> isize;
    pub fn LoadCursorW(inst: isize, name: *const u16) -> isize;
    pub fn GetSysColorBrush(i: i32) -> isize;
    pub fn GetSysColor(i: i32) -> u32;
    pub fn DrawTextW(dc: isize, s: *const u16, n: i32, rc: *mut RECT, f: u32) -> i32;
    pub fn AdjustWindowRectEx(rc: *mut RECT, style: u32, menu: BOOL, ex: u32) -> BOOL;
}

#[link(name = "gdi32", kind = "raw-dylib")]
extern "system" {
    #[allow(clippy::too_many_arguments)]
    pub fn CreateFontW(
        h: i32,
        w: i32,
        esc: i32,
        ori: i32,
        weight: i32,
        it: u32,
        ul: u32,
        so: u32,
        cs: u32,
        op: u32,
        cp: u32,
        q: u32,
        pf: u32,
        face: *const u16,
    ) -> isize;
    pub fn CreateDIBSection(
        dc: isize,
        bi: *const BITMAPINFO,
        usage: u32,
        bits: *mut *mut c_void,
        sec: isize,
        off: u32,
    ) -> isize;
    pub fn CreateBitmap(w: i32, h: i32, planes: u32, bpp: u32, bits: *const c_void) -> isize;
    pub fn DeleteObject(o: isize) -> BOOL;
    pub fn SelectObject(dc: isize, o: isize) -> isize;
    pub fn SetBkMode(dc: isize, m: i32) -> i32;
    pub fn SetTextColor(dc: isize, c: u32) -> u32;
    pub fn CreateSolidBrush(c: u32) -> isize;
}

#[link(name = "dxva2", kind = "raw-dylib")]
extern "system" {
    pub fn GetNumberOfPhysicalMonitorsFromHMONITOR(m: isize, n: *mut u32) -> BOOL;
    pub fn GetPhysicalMonitorsFromHMONITOR(m: isize, n: u32, arr: *mut PHYSICAL_MONITOR) -> BOOL;
    pub fn DestroyPhysicalMonitor(h: isize) -> BOOL;
    pub fn SetVCPFeature(h: isize, code: u8, v: u32) -> BOOL;
    pub fn GetVCPFeatureAndVCPFeatureReply(h: isize, code: u8, t: *mut u32, cur: *mut u32, max: *mut u32) -> BOOL;
    pub fn GetCapabilitiesStringLength(h: isize, len: *mut u32) -> BOOL;
    pub fn CapabilitiesRequestAndCapabilitiesReply(h: isize, s: *mut u8, len: u32) -> BOOL;
}

#[link(name = "shell32", kind = "raw-dylib")]
extern "system" {
    pub fn Shell_NotifyIconW(msg: u32, nid: *mut NOTIFYICONDATAW) -> BOOL;
    pub fn SHGetKnownFolderPath(id: *const GUID, flags: u32, token: isize, path: *mut *mut u16) -> i32;
}

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    pub fn RegDeleteKeyValueW(key: isize, sub: *const u16, name: *const u16) -> i32;
    pub fn GetUserNameW(buf: *mut u16, len: *mut u32) -> BOOL;
}

#[link(name = "ole32", kind = "raw-dylib")]
extern "system" {
    pub fn CoTaskMemFree(p: *mut c_void);
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> isize;
    pub fn CreateMutexW(attr: *const c_void, own: BOOL, name: *const u16) -> isize;
    pub fn GetLastError() -> u32;
    pub fn Sleep(ms: u32);
    pub fn CreateActCtxW(ctx: *const ACTCTXW) -> isize;
    pub fn ActivateActCtx(h: isize, cookie: *mut usize) -> BOOL;
    pub fn GetTickCount64() -> u64;
    pub fn GetLocalTime(t: *mut SYSTEMTIME);
    pub fn GetCurrentProcess() -> isize;
    pub fn GetProcessTimes(p: isize, c: *mut u64, e: *mut u64, k: *mut u64, u: *mut u64) -> BOOL;
    pub fn GetSystemTimeAsFileTime(t: *mut u64);
    pub fn GetUserDefaultUILanguage() -> u16;
}

pub const WM_NULL: u32 = 0x0000;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SETFOCUS: u32 = 0x0007;
pub const WM_KILLFOCUS: u32 = 0x0008;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_GETDLGCODE: u32 = 0x0087;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_SYSKEYDOWN: u32 = 0x0104;
pub const WM_SYSKEYUP: u32 = 0x0105;
pub const WM_SYSCHAR: u32 = 0x0106;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_HOTKEY: u32 = 0x0312;
pub const WM_APP: u32 = 0x8000;

pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_EX_DLGMODALFRAME: u32 = 0x0001;
pub const WS_EX_TOPMOST: u32 = 0x0008;
pub const WS_EX_TOOLWINDOW: u32 = 0x0080;
pub const WS_EX_CLIENTEDGE: u32 = 0x0200;

pub const BS_DEFPUSHBUTTON: u32 = 0x1;
pub const BS_AUTOCHECKBOX: u32 = 0x3;
pub const BS_GROUPBOX: u32 = 0x7;
pub const BS_MULTILINE: u32 = 0x2000;
pub const CBS_DROPDOWNLIST: u32 = 0x3;
pub const ES_AUTOHSCROLL: u32 = 0x80;
pub const SS_CENTER: u32 = 0x1;
pub const SS_CENTERIMAGE: u32 = 0x200;

pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;
pub const CB_ADDSTRING: u32 = 0x0143;
pub const CB_GETCURSEL: u32 = 0x0147;
pub const CB_SETCURSEL: u32 = 0x014E;

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;
pub const MOD_NOREPEAT: u32 = 0x4000;

pub const NIM_ADD: u32 = 0;
pub const NIM_MODIFY: u32 = 1;
pub const NIM_DELETE: u32 = 2;
pub const NIF_MESSAGE: u32 = 0x1;
pub const NIF_ICON: u32 = 0x2;
pub const NIF_TIP: u32 = 0x4;
pub const NIF_INFO: u32 = 0x10;
pub const NIIF_WARNING: u32 = 0x2;

pub const MF_SEPARATOR: u32 = 0x800;
pub const TPM_RIGHTBUTTON: u32 = 0x2;
pub const TPM_NONOTIFY: u32 = 0x80;
pub const TPM_RETURNCMD: u32 = 0x100;

pub const MB_ICONWARNING: u32 = 0x30;
pub const MB_ICONINFORMATION: u32 = 0x40;
pub const GWLP_USERDATA: i32 = -21;
pub const DLGC_WANTALLKEYS: isize = 0x4;
pub const IDOK: usize = 1;
pub const IDCANCEL: usize = 2;
pub const SW_SHOW: i32 = 5;
pub const SM_CXICON: i32 = 11;
pub const SM_CXSMICON: i32 = 49;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
pub const DPI_AWARENESS_CONTEXT_SYSTEM_AWARE: isize = -2;
pub const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;

/// NUL-terminated UTF-16 copy of `s`, for passing to `W` functions.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// String from a NUL-terminated UTF-16 buffer (the whole buffer if there is no NUL).
pub fn from_wide(b: &[u16]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf16_lossy(&b[..n])
}

/// Copies `s` into a fixed UTF-16 field, truncating it and always leaving the terminating NUL.
pub fn fill(dst: &mut [u16], s: &str) {
    let mut i = 0;
    for c in s.encode_utf16() {
        if i + 1 >= dst.len() {
            break;
        }
        dst[i] = c;
        i += 1;
    }
    dst[i] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_round_trip() {
        let w = wide("Swytch è");
        assert_eq!(w.last(), Some(&0));
        assert_eq!(from_wide(&w), "Swytch è");
    }

    #[test]
    fn fill_truncates_and_terminates() {
        let mut buf = [0xFFFFu16; 4];
        fill(&mut buf, "abcdef");
        assert_eq!(from_wide(&buf), "abc");
        assert_eq!(buf[3], 0);
    }
}
