//! Swytch — switch dell'ingresso del monitor via DDC/CI (VCP 0x60).
//!
//! - Hotkey globali (default AltGr+F13..F16), registrati come prima cosa all'avvio.
//! - Icona nel tray (aggiunta appena la taskbar esiste; riprova finche' Explorer non e' pronto).
//! - Finestra rapida PC/PS5 sul monitor secondario, finestra di configurazione con registratore di hotkey.
//! - Avvio al logon tramite attivita' pianificata (nessun ritardo di "Esecuzione automatica").
//!
//! Solo Win32 diretto, nessuna dipendenza. Tutta la UI (font, classi, stili visuali) e' creata
//! in modo pigro alla prima apertura di una finestra, cosi' l'avvio resta di pochi millisecondi.

#![windows_subsystem = "windows"]
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::fs;
use std::io::Write;
use std::mem::{size_of, zeroed};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::ptr::{null, null_mut};
use std::time::Instant;

// ───────────────────────────── Tipi e strutture Win32 ─────────────────────────────

type HWND = isize;
type BOOL = i32;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;
type WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;
type MONITORENUMPROC = Option<unsafe extern "system" fn(isize, isize, *mut RECT, LPARAM) -> BOOL>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct POINT { x: i32, y: i32 }

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RECT { left: i32, top: i32, right: i32, bottom: i32 }

#[repr(C)]
struct MSG { hwnd: HWND, message: u32, wParam: WPARAM, lParam: LPARAM, time: u32, pt: POINT, lPrivate: u32 }

#[repr(C)]
struct WNDCLASSEXW {
    cbSize: u32, style: u32, lpfnWndProc: WNDPROC, cbClsExtra: i32, cbWndExtra: i32,
    hInstance: isize, hIcon: isize, hCursor: isize, hbrBackground: isize,
    lpszMenuName: *const u16, lpszClassName: *const u16, hIconSm: isize,
}

#[repr(C)]
struct NOTIFYICONDATAW {
    cbSize: u32, hWnd: HWND, uID: u32, uFlags: u32, uCallbackMessage: u32, hIcon: isize,
    szTip: [u16; 128], dwState: u32, dwStateMask: u32, szInfo: [u16; 256], uVersion: u32,
    szInfoTitle: [u16; 64], dwInfoFlags: u32, guidItem: [u32; 4], hBalloonIcon: isize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PHYSICAL_MONITOR { hPhysicalMonitor: isize, szPhysicalMonitorDescription: [u16; 128] }

#[repr(C)]
struct MONITORINFOEXW { cbSize: u32, rcMonitor: RECT, rcWork: RECT, dwFlags: u32, szDevice: [u16; 32] }

#[repr(C)]
struct DISPLAY_DEVICEW {
    cb: u32, DeviceName: [u16; 32], DeviceString: [u16; 128], StateFlags: u32,
    DeviceID: [u16; 128], DeviceKey: [u16; 128],
}

#[repr(C)]
struct ICONINFO { fIcon: BOOL, xHotspot: u32, yHotspot: u32, hbmMask: isize, hbmColor: isize }

#[repr(C)]
struct BITMAPINFOHEADER {
    biSize: u32, biWidth: i32, biHeight: i32, biPlanes: u16, biBitCount: u16, biCompression: u32,
    biSizeImage: u32, biXPelsPerMeter: i32, biYPelsPerMeter: i32, biClrUsed: u32, biClrImportant: u32,
}

#[repr(C)]
struct BITMAPINFO { bmiHeader: BITMAPINFOHEADER, bmiColors: [u32; 1] }

#[repr(C)]
struct PAINTSTRUCT { hdc: isize, fErase: BOOL, rcPaint: RECT, fRestore: BOOL, fIncUpdate: BOOL, rgbReserved: [u8; 32] }

#[repr(C)]
struct ACTCTXW {
    cbSize: u32, dwFlags: u32, lpSource: *const u16, wProcessorArchitecture: u16, wLangId: u16,
    lpAssemblyDirectory: *const u16, lpResourceName: *const u16, lpApplicationName: *const u16, hModule: isize,
}

#[repr(C)]
#[derive(Default)]
struct SYSTEMTIME { wYear: u16, wMonth: u16, wDayOfWeek: u16, wDay: u16, wHour: u16, wMinute: u16, wSecond: u16, wMilliseconds: u16 }

// ───────────────────────────── Funzioni Win32 (raw-dylib: niente import lib) ─────────────────────────────

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    fn RegisterClassExW(wc: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32,
                       parent: HWND, menu: isize, inst: isize, param: *mut c_void) -> HWND;
    fn DefWindowProcW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    fn GetMessageW(msg: *mut MSG, h: HWND, a: u32, b: u32) -> BOOL;
    fn TranslateMessage(msg: *const MSG) -> BOOL;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn IsDialogMessageW(h: HWND, msg: *mut MSG) -> BOOL;
    fn PostQuitMessage(code: i32);
    fn RegisterHotKey(h: HWND, id: i32, mods: u32, vk: u32) -> BOOL;
    fn UnregisterHotKey(h: HWND, id: i32) -> BOOL;
    fn RegisterWindowMessageW(name: *const u16) -> u32;
    fn SetTimer(h: HWND, id: usize, ms: u32, f: *const c_void) -> usize;
    fn KillTimer(h: HWND, id: usize) -> BOOL;
    fn CreatePopupMenu() -> isize;
    fn AppendMenuW(m: isize, flags: u32, id: usize, text: *const u16) -> BOOL;
    fn TrackPopupMenu(m: isize, flags: u32, x: i32, y: i32, r: i32, h: HWND, rc: *const RECT) -> BOOL;
    fn DestroyMenu(m: isize) -> BOOL;
    fn SetForegroundWindow(h: HWND) -> BOOL;
    fn GetCursorPos(p: *mut POINT) -> BOOL;
    fn PostMessageW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> BOOL;
    fn SendMessageW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    fn DestroyWindow(h: HWND) -> BOOL;
    fn ShowWindow(h: HWND, cmd: i32) -> BOOL;
    fn FindWindowW(class: *const u16, title: *const u16) -> HWND;
    fn UpdateWindow(h: HWND) -> BOOL;
    fn SetWindowTextW(h: HWND, s: *const u16) -> BOOL;
    fn GetWindowTextW(h: HWND, s: *mut u16, n: i32) -> i32;
    fn MessageBoxW(h: HWND, text: *const u16, cap: *const u16, t: u32) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn CreateIconIndirect(ii: *const ICONINFO) -> isize;
    fn EnumDisplayMonitors(dc: isize, clip: *const RECT, f: MONITORENUMPROC, l: LPARAM) -> BOOL;
    fn GetMonitorInfoW(m: isize, mi: *mut MONITORINFOEXW) -> BOOL;
    fn EnumDisplayDevicesW(dev: *const u16, n: u32, dd: *mut DISPLAY_DEVICEW, flags: u32) -> BOOL;
    fn SetProcessDpiAwarenessContext(ctx: isize) -> BOOL;
    fn GetDpiForSystem() -> u32;
    fn GetKeyState(vk: i32) -> i16;
    fn InvalidateRect(h: HWND, rc: *const RECT, erase: BOOL) -> BOOL;
    fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> isize;
    fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    fn FillRect(dc: isize, rc: *const RECT, br: isize) -> i32;
    fn GetClientRect(h: HWND, rc: *mut RECT) -> BOOL;
    fn SetFocus(h: HWND) -> HWND;
    fn GetFocus() -> HWND;
    fn GetParent(h: HWND) -> HWND;
    fn GetNextDlgTabItem(dlg: HWND, ctl: HWND, prev: BOOL) -> HWND;
    fn SetWindowLongPtrW(h: HWND, i: i32, v: isize) -> isize;
    fn GetWindowLongPtrW(h: HWND, i: i32) -> isize;
    fn LoadCursorW(inst: isize, name: *const u16) -> isize;
    fn GetSysColorBrush(i: i32) -> isize;
    fn GetSysColor(i: i32) -> u32;
    fn DrawTextW(dc: isize, s: *const u16, n: i32, rc: *mut RECT, f: u32) -> i32;
    fn AdjustWindowRectEx(rc: *mut RECT, style: u32, menu: BOOL, ex: u32) -> BOOL;
}

#[link(name = "gdi32", kind = "raw-dylib")]
extern "system" {
    fn CreateFontW(h: i32, w: i32, esc: i32, ori: i32, weight: i32, it: u32, ul: u32, so: u32, cs: u32,
                   op: u32, cp: u32, q: u32, pf: u32, face: *const u16) -> isize;
    fn CreateDIBSection(dc: isize, bi: *const BITMAPINFO, usage: u32, bits: *mut *mut c_void, sec: isize, off: u32) -> isize;
    fn CreateBitmap(w: i32, h: i32, planes: u32, bpp: u32, bits: *const c_void) -> isize;
    fn DeleteObject(o: isize) -> BOOL;
    fn SelectObject(dc: isize, o: isize) -> isize;
    fn SetBkMode(dc: isize, m: i32) -> i32;
    fn SetTextColor(dc: isize, c: u32) -> u32;
    fn CreateSolidBrush(c: u32) -> isize;
}

#[link(name = "dxva2", kind = "raw-dylib")]
extern "system" {
    fn GetNumberOfPhysicalMonitorsFromHMONITOR(m: isize, n: *mut u32) -> BOOL;
    fn GetPhysicalMonitorsFromHMONITOR(m: isize, n: u32, arr: *mut PHYSICAL_MONITOR) -> BOOL;
    fn DestroyPhysicalMonitor(h: isize) -> BOOL;
    fn SetVCPFeature(h: isize, code: u8, v: u32) -> BOOL;
    fn GetVCPFeatureAndVCPFeatureReply(h: isize, code: u8, t: *mut u32, cur: *mut u32, max: *mut u32) -> BOOL;
    fn GetCapabilitiesStringLength(h: isize, len: *mut u32) -> BOOL;
    fn CapabilitiesRequestAndCapabilitiesReply(h: isize, s: *mut u8, len: u32) -> BOOL;
}

#[link(name = "shell32", kind = "raw-dylib")]
extern "system" {
    fn Shell_NotifyIconW(msg: u32, nid: *mut NOTIFYICONDATAW) -> BOOL;
}

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    fn RegDeleteKeyValueW(key: isize, sub: *const u16, name: *const u16) -> i32;
    fn GetUserNameW(buf: *mut u16, len: *mut u32) -> BOOL;
}

#[repr(C)]
struct GUID { d1: u32, d2: u16, d3: u16, d4: [u8; 8] }

#[link(name = "shell32", kind = "raw-dylib")]
extern "system" {
    fn SHGetKnownFolderPath(id: *const GUID, flags: u32, token: isize, path: *mut *mut u16) -> i32;
}

#[link(name = "ole32", kind = "raw-dylib")]
extern "system" {
    fn CoTaskMemFree(p: *mut c_void);
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn CreateMutexW(attr: *const c_void, own: BOOL, name: *const u16) -> isize;
    fn GetLastError() -> u32;
    fn Sleep(ms: u32);
    fn CreateActCtxW(ctx: *const ACTCTXW) -> isize;
    fn ActivateActCtx(h: isize, cookie: *mut usize) -> BOOL;
    fn GetTickCount64() -> u64;
    fn GetLocalTime(t: *mut SYSTEMTIME);
    fn GetCurrentProcess() -> isize;
    fn GetProcessTimes(p: isize, c: *mut u64, e: *mut u64, k: *mut u64, u: *mut u64) -> BOOL;
    fn GetSystemTimeAsFileTime(t: *mut u64);
}

// ───────────────────────────── Costanti ─────────────────────────────

const WM_NULL: u32 = 0x0000;
const WM_DESTROY: u32 = 0x0002;
const WM_SETFOCUS: u32 = 0x0007;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_SETFONT: u32 = 0x0030;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_GETDLGCODE: u32 = 0x0087;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_SYSCHAR: u32 = 0x0106;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_HOTKEY: u32 = 0x0312;
const WM_TRAY: u32 = 0x8000 + 1; // WM_APP + 1

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CAPTION: u32 = 0x00C0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_EX_DLGMODALFRAME: u32 = 0x0001;
const WS_EX_TOPMOST: u32 = 0x0008;
const WS_EX_TOOLWINDOW: u32 = 0x0080;
const WS_EX_CLIENTEDGE: u32 = 0x0200;

const BS_DEFPUSHBUTTON: u32 = 0x1;
const BS_AUTOCHECKBOX: u32 = 0x3;
const BS_GROUPBOX: u32 = 0x7;
const BS_MULTILINE: u32 = 0x2000;
const CBS_DROPDOWNLIST: u32 = 0x3;
const ES_AUTOHSCROLL: u32 = 0x80;
const SS_CENTER: u32 = 0x1;
const SS_CENTERIMAGE: u32 = 0x200;

const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;

const MOD_ALT: u32 = 0x1;
const MOD_CONTROL: u32 = 0x2;
const MOD_SHIFT: u32 = 0x4;
const MOD_WIN: u32 = 0x8;
const MOD_NOREPEAT: u32 = 0x4000;
const MOD_ALTGR: u32 = MOD_CONTROL | MOD_ALT; // AltGr sul layout italiano = Ctrl+Alt

const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE: u32 = 0x1;
const NIF_ICON: u32 = 0x2;
const NIF_TIP: u32 = 0x4;
const NIF_INFO: u32 = 0x10;
const NIIF_WARNING: u32 = 0x2;

const MF_SEPARATOR: u32 = 0x800;
const TPM_RIGHTBUTTON: u32 = 0x2;
const TPM_NONOTIFY: u32 = 0x80;
const TPM_RETURNCMD: u32 = 0x100;

const GWLP_USERDATA: i32 = -21;
const DLGC_WANTALLKEYS: isize = 0x4;
const VCP_INPUT: u8 = 0x60;
const IDOK: usize = 1;
const IDCANCEL: usize = 2;
const TIMER_TRAY: usize = 1;
const REC_FLAG: isize = 1 << 24; // nel campo hotkey: "combinazione appena registrata"

const HOTKEY_NAMES: [&str; 4] = ["DP", "HDMI", "Alterna", "Finestra"];
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const TASK_NAME: &str = "Swytch";
const APP_NAME: &str = "Swytch";
const LEGACY_NAME: &str = "monitorSwitch"; // versione precedente: config da migrare, avvio da rimuovere

// ───────────────────────────── Utilita' ─────────────────────────────

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

fn from_wide(b: &[u16]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf16_lossy(&b[..n])
}

fn fill(dst: &mut [u16], s: &str) {
    let mut i = 0;
    for c in s.encode_utf16() {
        if i + 1 >= dst.len() { break; }
        dst[i] = c;
        i += 1;
    }
    dst[i] = 0;
}

/// Cartella nota di Windows letta dalla shell (non dalle variabili d'ambiente:
/// un processo avviato dall'Utilita' di pianificazione puo' non avere APPDATA/LOCALAPPDATA).
fn known_folder(id: GUID, env_fallback: &str) -> PathBuf {
    let mut p: *mut u16 = null_mut();
    let base = unsafe {
        let ok = SHGetKnownFolderPath(&id, 0, 0, &mut p) == 0 && !p.is_null();
        let r = if ok {
            let mut n = 0;
            while *p.add(n) != 0 { n += 1; }
            Some(PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(p, n))))
        } else { None };
        if !p.is_null() { CoTaskMemFree(p as *mut c_void); }
        r
    };
    base.or_else(|| std::env::var_os(env_fallback).map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("."))
}

/// %APPDATA% (Roaming).
fn roaming() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| known_folder(
        // FOLDERID_RoamingAppData {3EB685DB-65F9-4CF6-A03A-E3EF65729F3D}
        GUID { d1: 0x3EB6_85DB, d2: 0x65F9, d3: 0x4CF6, d4: [0xA0, 0x3A, 0xE3, 0xEF, 0x65, 0x72, 0x9F, 0x3D] }, "APPDATA")).clone()
}

/// %APPDATA%\Swytch: config.ini e Swytch.log.
fn data_dir() -> PathBuf { roaming().join(APP_NAME) }

/// %LOCALAPPDATA%\Programs\Swytch: dove `Swytch.exe install` copia l'eseguibile.
fn install_dir() -> PathBuf {
    known_folder(
        // FOLDERID_LocalAppData {F1B32785-6FBA-4FCF-9D55-7B8E7F157091}
        GUID { d1: 0xF1B3_2785, d2: 0x6FBA, d3: 0x4FCF, d4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91] }, "LOCALAPPDATA")
        .join("Programs").join(APP_NAME)
}

fn user_name() -> String {
    let mut buf = [0u16; 257];
    let mut len = buf.len() as u32;
    if unsafe { GetUserNameW(buf.as_mut_ptr(), &mut len) } != 0 { from_wide(&buf) }
    else { std::env::var("USERNAME").unwrap_or_default() }
}

fn log(msg: &str) {
    let mut t = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut t) };
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(dir.join("Swytch.log")) {
        let _ = writeln!(f, "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}  {}",
                         t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds, msg);
    }
}

fn parse_num(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn ms_since_process_start() -> u64 {
    unsafe {
        let (mut c, mut e, mut k, mut u, mut now) = (0u64, 0u64, 0u64, 0u64, 0u64);
        GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u);
        GetSystemTimeAsFileTime(&mut now);
        now.saturating_sub(c) / 10_000
    }
}

// ───────────────────────────── Hotkey ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
struct Hotkey { m: u32, vk: u32 }

impl Hotkey {
    const NONE: Hotkey = Hotkey { m: 0, vk: 0 };
    fn is_set(&self) -> bool { self.vk != 0 }
    fn pack(&self) -> isize { ((self.m as isize) << 16) | self.vk as isize }
    fn unpack(v: isize) -> Hotkey { Hotkey { m: ((v >> 16) & 0xFF) as u32, vk: (v & 0xFFFF) as u32 } }

    fn label(&self) -> String {
        if !self.is_set() { return "(nessuno)".into(); }
        let mut p: Vec<String> = Vec::new();
        let ctrl = self.m & MOD_CONTROL != 0;
        let alt = self.m & MOD_ALT != 0;
        if ctrl && alt { p.push("AltGr".into()); } else {
            if ctrl { p.push("Ctrl".into()); }
            if alt { p.push("Alt".into()); }
        }
        if self.m & MOD_SHIFT != 0 { p.push("Shift".into()); }
        if self.m & MOD_WIN != 0 { p.push("Win".into()); }
        p.push(key_name(self.vk));
        p.join("+")
    }

    fn serialize(&self) -> String { format!("0x{:02X},0x{:02X}", self.m, self.vk) }

    fn parse(s: &str, fb: Hotkey) -> Hotkey {
        match s.split_once(',') {
            Some((a, b)) => Hotkey {
                m: parse_num(a).map(|v| v as u32).unwrap_or(fb.m),
                vk: parse_num(b).map(|v| v as u32).unwrap_or(fb.vk),
            },
            None => fb,
        }
    }
}

fn key_name(vk: u32) -> String {
    match vk {
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x30..=0x39 | 0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        0x08 => "Backspace".into(), 0x09 => "Tab".into(), 0x0D => "Invio".into(), 0x13 => "Pausa".into(),
        0x1B => "Esc".into(), 0x20 => "Spazio".into(), 0x21 => "PagSu".into(), 0x22 => "PagGiu".into(),
        0x23 => "Fine".into(), 0x24 => "Home".into(), 0x25 => "Sinistra".into(), 0x26 => "Su".into(),
        0x27 => "Destra".into(), 0x28 => "Giu".into(), 0x2C => "Stamp".into(), 0x2D => "Ins".into(),
        0x2E => "Canc".into(), 0x6A => "Num*".into(), 0x6B => "Num+".into(), 0x6D => "Num-".into(),
        0x6E => "Num.".into(), 0x6F => "Num/".into(), 0x91 => "BlocScorr".into(), 0xAD => "Muto".into(),
        0xAE => "Vol-".into(), 0xAF => "Vol+".into(), 0xB0 => "Succ".into(), 0xB1 => "Prec".into(),
        0xB3 => "Play".into(),
        _ => format!("VK 0x{:02X}", vk),
    }
}

// ───────────────────────────── Configurazione ─────────────────────────────

#[derive(Clone)]
struct Config { mon: i32, dp: u32, hdmi: u32, autostart: bool, hotkeys: bool, hk: [Hotkey; 4] }

impl Config {
    fn defaults() -> Config {
        Config {
            mon: -1, dp: 0x0F, hdmi: 0x11, autostart: true, hotkeys: true,
            hk: [
                Hotkey { m: MOD_ALTGR, vk: 0x7C }, // F13
                Hotkey { m: MOD_ALTGR, vk: 0x7D }, // F14
                Hotkey { m: MOD_ALTGR, vk: 0x7E }, // F15
                Hotkey { m: MOD_ALTGR, vk: 0x7F }, // F16
            ],
        }
    }

    fn path() -> PathBuf { data_dir().join("config.ini") }

    fn load() -> (Config, bool) {
        let mut c = Config::defaults();
        // Migrazione dalla versione precedente (%APPDATA%\monitorSwitch\config.ini).
        if !Config::path().exists() {
            let old = roaming().join(LEGACY_NAME).join("config.ini");
            if old.exists() {
                let _ = fs::create_dir_all(data_dir());
                let _ = fs::copy(&old, Config::path());
            }
        }
        let text = match fs::read_to_string(Config::path()) { Ok(t) => t, Err(_) => return (c, false) };
        let d = Config::defaults();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            let flag = v == "1" || v.eq_ignore_ascii_case("true");
            match k.trim() {
                "MonitorIndex" => c.mon = parse_num(v).map(|x| x as i32).unwrap_or(-1),
                "DpCode" => c.dp = parse_num(v).map(|x| x as u32).unwrap_or(d.dp),
                "HdmiCode" => c.hdmi = parse_num(v).map(|x| x as u32).unwrap_or(d.hdmi),
                "AutoStart" => c.autostart = flag,
                "HotkeysEnabled" => c.hotkeys = flag,
                "HotkeyDp" => c.hk[0] = Hotkey::parse(v, d.hk[0]),
                "HotkeyHdmi" => c.hk[1] = Hotkey::parse(v, d.hk[1]),
                "HotkeyToggle" => c.hk[2] = Hotkey::parse(v, d.hk[2]),
                "HotkeyWindow" => c.hk[3] = Hotkey::parse(v, d.hk[3]),
                _ => {}
            }
        }
        (c, true)
    }

    fn save(&self) {
        let _ = fs::create_dir_all(data_dir());
        let s = format!(
            "MonitorIndex={}\r\nDpCode=0x{:02X}\r\nHdmiCode=0x{:02X}\r\nAutoStart={}\r\nHotkeysEnabled={}\r\n\
             ; hotkey = modificatori,tasto  (0x03 = AltGr/Ctrl+Alt, 0x7C = F13)\r\n\
             HotkeyDp={}\r\nHotkeyHdmi={}\r\nHotkeyToggle={}\r\nHotkeyWindow={}\r\n",
            self.mon, self.dp, self.hdmi, self.autostart as u8, self.hotkeys as u8,
            self.hk[0].serialize(), self.hk[1].serialize(), self.hk[2].serialize(), self.hk[3].serialize());
        let _ = fs::write(Config::path(), s);
    }
}

// ───────────────────────────── DDC/CI ─────────────────────────────

struct Phys { h: isize, name: String }

unsafe extern "system" fn enum_mon_cb(h: isize, _dc: isize, _rc: *mut RECT, l: LPARAM) -> BOOL {
    (*(l as *mut Vec<isize>)).push(h);
    1
}

fn hmonitors() -> Vec<isize> {
    let mut v: Vec<isize> = Vec::new();
    unsafe { EnumDisplayMonitors(0, null(), Some(enum_mon_cb), &mut v as *mut Vec<isize> as LPARAM) };
    v
}

fn vendor(pnp: &str) -> &'static str {
    match pnp {
        "LEN" => "Lenovo", "SAM" | "SEC" => "Samsung", "DEL" => "Dell", "GSM" => "LG", "ACI" | "AUS" => "Asus",
        "BNQ" => "BenQ", "AOC" => "AOC", "HWP" | "HPN" => "HP", "PHL" => "Philips", "GBT" => "Gigabyte",
        "MSI" => "MSI", "ACR" => "Acer", "VSC" => "ViewSonic", "IVM" => "Iiyama", "EIZ" | "ENC" => "Eizo",
        "SNY" => "Sony", "AUO" => "AUO", "BOE" => "BOE", "XMI" => "Xiaomi", "HKC" => "HKC",
        _ => "",
    }
}

/// Nome leggibile del monitor collegato a un HMONITOR, es. "Lenovo (LEN67C5)".
fn monitor_name(hmon: isize) -> String {
    unsafe {
        let mut mi: MONITORINFOEXW = zeroed();
        mi.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut mi) == 0 { return "Monitor".into(); }
        let mut dd: DISPLAY_DEVICEW = zeroed();
        dd.cb = size_of::<DISPLAY_DEVICEW>() as u32;
        if EnumDisplayDevicesW(mi.szDevice.as_ptr(), 0, &mut dd, 0) == 0 { return from_wide(&mi.szDevice); }
        // DeviceID = "MONITOR\LEN67C5\{...}\0001"
        let id = from_wide(&dd.DeviceID);
        let code = id.split('\\').nth(1).unwrap_or("").to_string();
        if code.len() >= 3 {
            let v = vendor(&code[..3]);
            if !v.is_empty() { return format!("{} ({})", v, code); }
            return code;
        }
        from_wide(&dd.DeviceString)
    }
}

fn phys_monitors() -> Vec<Phys> {
    let mut out = Vec::new();
    for hm in hmonitors() {
        unsafe {
            let mut n = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hm, &mut n) == 0 || n == 0 { continue; }
            let mut arr = vec![PHYSICAL_MONITOR { hPhysicalMonitor: 0, szPhysicalMonitorDescription: [0; 128] }; n as usize];
            if GetPhysicalMonitorsFromHMONITOR(hm, n, arr.as_mut_ptr()) == 0 { continue; }
            let name = monitor_name(hm);
            for p in arr { out.push(Phys { h: p.hPhysicalMonitor, name: name.clone() }); }
        }
    }
    out
}

fn release(list: &[Phys]) {
    for p in list { unsafe { DestroyPhysicalMonitor(p.h) }; }
}

fn vcp_get(h: isize) -> Option<u32> {
    let (mut t, mut cur, mut max) = (0u32, 0u32, 0u32);
    if unsafe { GetVCPFeatureAndVCPFeatureReply(h, VCP_INPUT, &mut t, &mut cur, &mut max) } != 0 { Some(cur) } else { None }
}

/// Monitor scelto: indice fisso, oppure (auto) il primo che risponde a VCP 0x60.
fn pick(list: &[Phys], idx: i32) -> Option<&Phys> {
    if idx >= 0 && (idx as usize) < list.len() { return Some(&list[idx as usize]); }
    list.iter().find(|p| vcp_get(p.h).is_some())
}

fn ddc_set(idx: i32, value: u32) -> Result<String, String> {
    let list = phys_monitors();
    let r = match pick(&list, idx) {
        None => Err("nessun monitor compatibile DDC/CI".to_string()),
        Some(p) => {
            if unsafe { SetVCPFeature(p.h, VCP_INPUT, value) } != 0 { Ok(p.name.clone()) }
            else { Err(format!("{} non ha accettato il comando", p.name)) }
        }
    };
    release(&list);
    r
}

fn ddc_get(idx: i32) -> Option<u32> {
    let list = phys_monitors();
    let r = pick(&list, idx).and_then(|p| vcp_get(p.h));
    release(&list);
    r
}

fn supported_inputs(h: isize) -> Option<String> {
    unsafe {
        let mut len = 0u32;
        if GetCapabilitiesStringLength(h, &mut len) == 0 || len == 0 { return None; }
        let mut buf = vec![0u8; len as usize];
        if CapabilitiesRequestAndCapabilitiesReply(h, buf.as_mut_ptr(), len) == 0 { return None; }
        let caps = String::from_utf8_lossy(&buf);
        let b = caps.as_bytes();
        let mut from = 0;
        while let Some(pos) = caps[from..].find("60(") {
            let at = from + pos;
            if at == 0 || b[at - 1] == b' ' || b[at - 1] == b'(' {
                let rest = &caps[at + 3..];
                return rest.find(')').map(|e| rest[..e].trim().to_string());
            }
            from = at + 3;
        }
        None
    }
}

fn ddc_report() -> String {
    let list = phys_monitors();
    let mut s = String::new();
    for (i, p) in list.iter().enumerate() {
        s.push_str(&format!("Monitor #{}: {}\n", i, p.name));
        match vcp_get(p.h) {
            Some(v) => s.push_str(&format!("    ingresso attuale = 0x{:02X}\n", v)),
            None => s.push_str("    ingresso: non supportato (DDC/CI assente)\n"),
        }
        if let Some(c) = supported_inputs(p.h) { s.push_str(&format!("    codici ingresso supportati (60): {}\n", c)); }
        s.push('\n');
    }
    release(&list);
    if s.is_empty() { s = "Nessun monitor rilevato.".into(); }
    s
}

// ───────────────────────────── Avvio al logon (attivita' pianificata) ─────────────────────────────

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn run_hidden(exe: &str, args: &[&std::ffi::OsStr]) -> bool {
    Command::new(exe).args(args).creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x.to_string_lossy().eq_ignore_ascii_case(&y.to_string_lossy()),
        _ => false,
    }
}

/// `Swytch.exe install`: copia l'eseguibile in %LOCALAPPDATA%\Programs\Swytch, crea l'attivita'
/// al logon, rimuove l'avvio della versione precedente e avvia l'app.
fn install() -> bool {
    let Ok(cur) = std::env::current_exe() else { return false };
    let dir = install_dir();
    let target = dir.join(format!("{}.exe", APP_NAME));
    if !same_file(&cur, &target) {
        // Un'istanza gia' in esecuzione blocca il file: chiedile di uscire (aggiornamento).
        unsafe {
            let h = FindWindowW(wide("SwytchMain").as_ptr(), null());
            if h != 0 {
                PostMessageW(h, WM_COMMAND, 6, 0);
                for _ in 0..40 { if FindWindowW(wide("SwytchMain").as_ptr(), null()) == 0 { break; } Sleep(50); }
            }
        }
        let _ = fs::create_dir_all(&dir);
        if let Err(e) = fs::copy(&cur, &target) {
            log(&format!("Installazione: copia in {:?} fallita: {}", target, e));
            return false;
        }
    }
    // Versione precedente ("monitorSwitch"): attivita' e voce Run.
    run_hidden("schtasks.exe", &["/Delete".as_ref(), "/TN".as_ref(), LEGACY_NAME.as_ref(), "/F".as_ref()]);
    let hkcu = 0x8000_0001u32 as i32 as isize;
    unsafe { RegDeleteKeyValueW(hkcu, wide(RUN_KEY).as_ptr(), wide(LEGACY_NAME).as_ptr()) };

    let ok = install_task(&target);
    if ok { run_hidden("schtasks.exe", &["/Run".as_ref(), "/TN".as_ref(), TASK_NAME.as_ref()]); }
    log(&format!("Installazione in {:?}: {}", target, if ok { "OK" } else { "FALLITA" }));
    ok
}

fn install_task(exe: &std::path::Path) -> bool {
    let workdir = exe.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let user = user_name();
    let xml = format!(
r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Swytch: switch ingresso monitor via DDC/CI (avvio immediato al logon)</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>"{exe}"</Command><WorkingDirectory>{workdir}</WorkingDirectory></Exec></Actions>
</Task>
"#, user = xml_esc(&user), exe = xml_esc(&exe.to_string_lossy()), workdir = xml_esc(&workdir));

    let _ = fs::create_dir_all(data_dir());
    let path = data_dir().join("task.xml");
    let mut bytes = vec![0xFFu8, 0xFE];
    for u in xml.encode_utf16() { bytes.extend_from_slice(&u.to_le_bytes()); }
    if fs::write(&path, &bytes).is_err() { return false; }
    let ok = run_hidden("schtasks.exe", &["/Create".as_ref(), "/TN".as_ref(), TASK_NAME.as_ref(),
                                          "/XML".as_ref(), path.as_os_str(), "/F".as_ref()]);
    let _ = fs::remove_file(&path);
    if ok {
        // L'attivita' sostituisce la vecchia voce "Run" (che parte in ritardo).
        let hkcu = 0x8000_0001u32 as i32 as isize;
        unsafe { RegDeleteKeyValueW(hkcu, wide(RUN_KEY).as_ptr(), wide(APP_NAME).as_ptr()) };
    }
    log(&format!("Avvio al logon (attivita' pianificata): installazione {}", if ok { "OK" } else { "FALLITA" }));
    ok
}

fn uninstall_task() -> bool {
    let ok = run_hidden("schtasks.exe", &["/Delete".as_ref(), "/TN".as_ref(), TASK_NAME.as_ref(), "/F".as_ref()]);
    log(&format!("Avvio al logon: rimozione {}", if ok { "OK" } else { "FALLITA" }));
    ok
}

// ───────────────────────────── Stato applicazione ─────────────────────────────

#[derive(Clone, Copy)]
struct CfgUi { combo: HWND, dp: HWND, hdmi: HWND, chk_hk: HWND, chk_auto: HWND, boxes: [HWND; 4] }

struct App {
    hinst: isize,
    main: HWND,
    cfg: Config,
    icon: isize,
    tray: bool,
    wm_taskbar: u32,
    dpi: u32,
    t0: Instant,
    ui_ready: bool,
    font: isize,
    font_big: isize,
    quick: HWND,
    quick_status: HWND,
    config: HWND,
    ui: Option<CfgUi>,
}

static mut APP: *mut App = null_mut();

#[allow(clippy::mut_from_ref)]
fn app() -> &'static mut App { unsafe { &mut *APP } }

fn s(v: i32) -> i32 { v * app().dpi as i32 / 96 }

// ───────────────────────────── Main ─────────────────────────────

fn cli(arg: &str) -> i32 {
    let (cfg, _) = Config::load();
    match arg {
        "dp" | "displayport" | "pc" => ddc_set(cfg.mon, cfg.dp).is_ok() as i32 ^ 1,
        "hdmi" | "ps5" => ddc_set(cfg.mon, cfg.hdmi).is_ok() as i32 ^ 1,
        "toggle" => {
            let target = if ddc_get(cfg.mon) == Some(cfg.dp) { cfg.hdmi } else { cfg.dp };
            ddc_set(cfg.mon, target).is_ok() as i32 ^ 1
        }
        "install" => install() as i32 ^ 1,
        "uninstall" => uninstall_task() as i32 ^ 1,
        _ => 2,
    }
}

fn main() {
    let t0 = Instant::now();
    if let Some(a) = std::env::args().nth(1) {
        if a.eq_ignore_ascii_case("icon-preview") {
            std::process::exit(icon_preview(&std::env::args().nth(2).unwrap_or_else(|| "icon-preview.bmp".into())));
        }
        std::process::exit(cli(&a.to_ascii_lowercase()));
    }

    unsafe {
        // Istanza unica (stesso nome usato anche dalla vecchia versione C#).
        CreateMutexW(null(), 0, wide("Swytch_SingleInstance").as_ptr());
        if GetLastError() == 183 { return; } // ERROR_ALREADY_EXISTS

        SetProcessDpiAwarenessContext(-2); // DPI_AWARENESS_CONTEXT_SYSTEM_AWARE
        let hinst = GetModuleHandleW(null());
        let (cfg, existed) = Config::load();
        if !existed { cfg.save(); }

        APP = Box::into_raw(Box::new(App {
            hinst, main: 0, cfg, icon: 0, tray: false, wm_taskbar: 0, dpi: GetDpiForSystem().max(96), t0,
            ui_ready: false, font: 0, font_big: 0, quick: 0, quick_status: 0, config: 0, ui: None,
        }));

        // Finestra nascosta (top-level, cosi' riceve anche il broadcast "TaskbarCreated").
        let cls = wide("SwytchMain");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32, style: 0, lpfnWndProc: Some(main_proc), cbClsExtra: 0,
            cbWndExtra: 0, hInstance: hinst, hIcon: 0, hCursor: 0, hbrBackground: 0, lpszMenuName: null(),
            lpszClassName: cls.as_ptr(), hIconSm: 0,
        };
        RegisterClassExW(&wc);
        let main = CreateWindowExW(0, cls.as_ptr(), wide("Swytch").as_ptr(), 0, 0, 0, 0, 0, 0, 0, hinst, null_mut());
        let a = app();
        a.main = main;

        // 1) Hotkey subito: funzionano anche prima che Explorer/taskbar siano pronti.
        apply_hotkeys();
        log(&format!("Avvio: hotkey pronti dopo {} ms dalla creazione del processo (main {} ms, PC acceso da {:.1} s)",
                     ms_since_process_start(), a.t0.elapsed().as_millis(), GetTickCount64() as f64 / 1000.0));

        // 2) Icona nel tray: se la taskbar non c'e' ancora, riprova ogni 250 ms e su "TaskbarCreated".
        a.wm_taskbar = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        a.icon = make_icon(GetSystemMetrics(49)); // SM_CXSMICON
        if add_tray() { log_tray_ready(); } else { SetTimer(main, TIMER_TRAY, 250, null()); }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            let a = app();
            if a.config != 0 && IsDialogMessageW(a.config, &mut msg) != 0 { continue; }
            if a.quick != 0 && IsDialogMessageW(a.quick, &mut msg) != 0 { continue; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn log_tray_ready() {
    log(&format!("Icona tray aggiunta dopo {} ms dalla creazione del processo", ms_since_process_start()));
}

unsafe extern "system" fn main_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let a = app();
    if a.wm_taskbar != 0 && msg == a.wm_taskbar {
        // Explorer (ri)partito: va ri-aggiunta l'icona.
        a.tray = false;
        if add_tray() { KillTimer(h, TIMER_TRAY); } else { SetTimer(h, TIMER_TRAY, 250, null()); }
        return 0;
    }
    match msg {
        WM_HOTKEY => {
            match wp {
                1 => switch_to(false),
                2 => switch_to(true),
                3 => toggle(),
                4 => show_quick(),
                _ => {}
            }
            0
        }
        WM_TRAY => {
            let ev = (lp as u32) & 0xFFFF;
            if ev == WM_RBUTTONUP || ev == WM_CONTEXTMENU { show_menu(); } else if ev == WM_LBUTTONDBLCLK { show_quick(); }
            0
        }
        WM_COMMAND => { menu_cmd(wp & 0xFFFF); 0 }
        WM_TIMER if wp == TIMER_TRAY => {
            if add_tray() { KillTimer(h, TIMER_TRAY); log_tray_ready(); }
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

// ───────────────────────────── Azioni ─────────────────────────────

fn apply_hotkeys() {
    let a = app();
    for id in 1..=4 { unsafe { UnregisterHotKey(a.main, id) }; }
    if !a.cfg.hotkeys { log("Hotkey disabilitati"); return; }
    let mut parts = Vec::new();
    let mut all = true;
    for (i, hk) in a.cfg.hk.iter().enumerate() {
        if !hk.is_set() { parts.push(format!("{}=(nessuno)", HOTKEY_NAMES[i])); continue; }
        let ok = unsafe { RegisterHotKey(a.main, i as i32 + 1, hk.m | MOD_NOREPEAT, hk.vk) } != 0;
        all &= ok;
        parts.push(format!("{}[{}]={}", HOTKEY_NAMES[i], hk.label(), if ok { "OK" } else { "FALLITO" }));
    }
    log(&format!("Hotkey: {}", parts.join("  ")));
    if !all { balloon("Alcuni hotkey non sono stati registrati (gia' in uso da un'altra app?)"); }
}

fn switch_to(hdmi: bool) {
    let a = app();
    let (code, label) = if hdmi { (a.cfg.hdmi, "HDMI (PS5)") } else { (a.cfg.dp, "DisplayPort (PC)") };
    match ddc_set(a.cfg.mon, code) {
        Ok(name) => log(&format!("OK   -> {}  [{}]", label, name)),
        Err(e) => {
            log(&format!("FAIL -> {}  [{}]", label, e));
            balloon(&format!("Switch a {} fallito: {}", label, e));
        }
    }
    set_tip(&format!("Swytch - ultimo: {}", label));
}

fn toggle() {
    let a = app();
    let cur = ddc_get(a.cfg.mon);
    switch_to(cur == Some(a.cfg.dp));
}

fn current_label() -> String {
    let a = app();
    match ddc_get(a.cfg.mon) {
        Some(v) if v == a.cfg.dp => "PC (DisplayPort)".into(),
        Some(v) if v == a.cfg.hdmi => "PS5 (HDMI)".into(),
        Some(v) => format!("sconosciuto (0x{:02X})", v),
        None => "non leggibile".into(),
    }
}

fn exit_app() {
    let a = app();
    unsafe {
        for id in 1..=4 { UnregisterHotKey(a.main, id); }
        let mut n = nid();
        Shell_NotifyIconW(NIM_DELETE, &mut n);
        log("Uscita");
        PostQuitMessage(0);
    }
}

// ───────────────────────────── Tray ─────────────────────────────

fn nid() -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = unsafe { zeroed() };
    n.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = app().main;
    n.uID = 1;
    n
}

fn add_tray() -> bool {
    let a = app();
    if a.tray { return true; }
    let mut n = nid();
    n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = a.icon;
    fill(&mut n.szTip, "Swytch");
    a.tray = unsafe { Shell_NotifyIconW(NIM_ADD, &mut n) } != 0;
    a.tray
}

fn set_tip(text: &str) {
    if !app().tray { return; }
    let mut n = nid();
    n.uFlags = NIF_TIP;
    fill(&mut n.szTip, text);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &mut n) };
}

fn balloon(text: &str) {
    if !app().tray { return; }
    let mut n = nid();
    n.uFlags = NIF_INFO;
    n.dwInfoFlags = NIIF_WARNING;
    fill(&mut n.szInfoTitle, "Swytch");
    fill(&mut n.szInfo, text);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &mut n) };
}

fn show_menu() {
    let cfg = app().cfg.clone();
    let main = app().main;
    let hint = |i: usize| if cfg.hotkeys && cfg.hk[i].is_set() { format!("\t{}", cfg.hk[i].label()) } else { String::new() };
    unsafe {
        let m = CreatePopupMenu();
        AppendMenuW(m, 0, 1, wide(&format!("PC — DisplayPort{}", hint(0))).as_ptr());
        AppendMenuW(m, 0, 2, wide(&format!("PS5 — HDMI{}", hint(1))).as_ptr());
        AppendMenuW(m, 0, 3, wide(&format!("Alterna{}", hint(2))).as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, 0, 4, wide(&format!("Finestra rapida{}", hint(3))).as_ptr());
        AppendMenuW(m, 0, 5, wide("Configura...").as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, 0, 6, wide("Esci").as_ptr());
        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        SetForegroundWindow(main);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, main, null());
        PostMessageW(main, WM_NULL, 0, 0);
        DestroyMenu(m);
        menu_cmd(cmd as usize);
    }
}

/// Comandi del menu del tray (anche via WM_COMMAND alla finestra principale).
fn menu_cmd(cmd: usize) {
    match cmd {
        1 => switch_to(false),
        2 => switch_to(true),
        3 => toggle(),
        4 => show_quick(),
        5 => show_config(),
        6 => exit_app(),
        _ => {}
    }
}

// ───── Icona: renderizzata a runtime con bordi anti-aliasing (SDF), nitida a ogni dimensione ─────

/// Distanza con segno da un rettangolo arrotondato (centro cx,cy, semi-lati hw,hh, raggio r).
fn sd_rrect(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - hw + r;
    let qy = (y - cy).abs() - hh + r;
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Pixel ARGB (alpha non premoltiplicato) dell'icona di lato n.
/// Disegno in uno spazio 32x32: tessera arrotondata con gradiente blu→viola + monitor bianco.
fn render_icon(n: i32) -> Vec<u32> {
    let k = n as f32 / 32.0; // pixel per unita'
    let cov = |d: f32| (0.5 - d * k).clamp(0.0, 1.0);
    let top = [0x4Fu8 as f32, 0x8C as f32, 0xFF as f32];   // #4F8CFF
    let bot = [0x7Cu8 as f32, 0x3A as f32, 0xED as f32];   // #7C3AED
    let white = [255.0f32, 255.0, 255.0];
    let mut out = vec![0u32; (n * n) as usize];
    for py in 0..n {
        for px in 0..n {
            let x = (px as f32 + 0.5) / k;
            let y = (py as f32 + 0.5) / k;
            let mut c = [0.0f32; 4]; // rgb premoltiplicato + alpha
            let mut over = |col: [f32; 3], a: f32| {
                if a <= 0.0 { return; }
                for i in 0..3 { c[i] = col[i] * a + c[i] * (1.0 - a); }
                c[3] = a + c[3] * (1.0 - a);
            };
            // tessera
            let t = ((x + y) / 64.0).clamp(0.0, 1.0);
            let grad = [top[0] + (bot[0] - top[0]) * t, top[1] + (bot[1] - top[1]) * t, top[2] + (bot[2] - top[2]) * t];
            over(grad, cov(sd_rrect(x, y, 16.0, 16.0, 15.5, 15.5, 7.5)));
            // monitor: cornice, vetro, collo, base
            let body = sd_rrect(x, y, 16.0, 14.0, 9.5, 6.75, 2.4);
            over(white, 0.16 * cov(body + 1.15));
            over(white, cov(body.abs() - 1.15));
            over(white, cov(sd_rrect(x, y, 16.0, 22.4, 1.3, 1.7, 0.3)));
            over(white, cov(sd_rrect(x, y, 16.0, 24.5, 5.0, 1.15, 1.15)));
            let a = c[3];
            if a > 0.0 {
                let ch = |v: f32| ((v / a).round().clamp(0.0, 255.0)) as u32;
                out[(py * n + px) as usize] = ((a * 255.0).round() as u32) << 24 | ch(c[0]) << 16 | ch(c[1]) << 8 | ch(c[2]);
            }
        }
    }
    out
}

/// Anteprima dell'icona (BMP 32 bit) a varie dimensioni su sfondo chiaro e scuro: `Swytch icon-preview file.bmp`.
fn icon_preview(path: &str) -> i32 {
    let sizes = [16, 20, 24, 32, 48, 64, 128];
    let pad = 16;
    let w: i32 = sizes.iter().sum::<i32>() + pad * (sizes.len() as i32 + 1);
    let h = (128 + pad * 2) * 2;
    let mut img = vec![0u32; (w * h) as usize];
    for (row, bg) in [(0, 0xF3u32), (1, 0x20u32)] {
        let y0 = row * (128 + pad * 2);
        for y in y0..y0 + 128 + pad * 2 { for x in 0..w { img[(y * w + x) as usize] = bg << 16 | bg << 8 | bg; } }
        let mut x0 = pad;
        for &n in &sizes {
            let px = render_icon(n);
            let oy = y0 + pad + (128 - n) / 2;
            for y in 0..n {
                for x in 0..n {
                    let p = px[(y * n + x) as usize];
                    let a = (p >> 24) as f32 / 255.0;
                    let d = &mut img[((oy + y) * w + x0 + x) as usize];
                    let mix = |s: u32, sh: u32| ((((p >> sh) & 0xFF) as f32 * a + s as f32 * (1.0 - a)).round() as u32) << sh;
                    *d = mix(bg, 16) | mix(bg, 8) | mix(bg, 0);
                }
            }
            x0 += n + pad;
        }
    }
    let mut f = Vec::with_capacity(54 + img.len() * 4);
    let size = 54 + img.len() as u32 * 4;
    f.extend_from_slice(b"BM"); f.extend_from_slice(&size.to_le_bytes()); f.extend_from_slice(&[0; 4]); f.extend_from_slice(&54u32.to_le_bytes());
    f.extend_from_slice(&40u32.to_le_bytes()); f.extend_from_slice(&w.to_le_bytes()); f.extend_from_slice(&(-h).to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes()); f.extend_from_slice(&32u16.to_le_bytes()); f.extend_from_slice(&[0; 24]);
    for p in img { f.extend_from_slice(&(p | 0xFF00_0000).to_le_bytes()); }
    fs::write(path, f).is_ok() as i32 ^ 1
}

/// Icona di lato n (niente file .ico, niente risorse).
fn make_icon(n: i32) -> isize {
    unsafe {
        let n = n.max(16);
        let mut bi: BITMAPINFO = zeroed();
        bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = n;
        bi.bmiHeader.biHeight = -n; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        let mut bits: *mut c_void = null_mut();
        let color = CreateDIBSection(0, &bi, 0, &mut bits, 0, 0);
        if color == 0 || bits.is_null() { return 0; }
        let px = std::slice::from_raw_parts_mut(bits as *mut u32, (n * n) as usize);
        px.copy_from_slice(&render_icon(n));
        let mask_bytes = vec![0u8; (((n + 15) / 16) * 2 * n) as usize];
        let mask = CreateBitmap(n, n, 1, 1, mask_bytes.as_ptr() as *const c_void);
        let ii = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&ii);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}

// ───────────────────────────── UI (creata in modo pigro) ─────────────────────────────

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>
"#;

fn ensure_ui() {
    let a = app();
    if a.ui_ready { return; }
    unsafe {
        // Stili visuali moderni (Common Controls v6) tramite activation context, solo quando serve la UI.
        let path = data_dir().join("visualstyles.manifest");
        if fs::read_to_string(&path).map(|t| t != MANIFEST).unwrap_or(true) {
            let _ = fs::create_dir_all(data_dir());
            let _ = fs::write(&path, MANIFEST);
        }
        let wpath = wide(&path.to_string_lossy());
        let mut ctx: ACTCTXW = zeroed();
        ctx.cbSize = size_of::<ACTCTXW>() as u32;
        ctx.lpSource = wpath.as_ptr();
        let h = CreateActCtxW(&ctx);
        if h != -1 { let mut cookie = 0usize; ActivateActCtx(h, &mut cookie); }

        let face = wide("Segoe UI");
        a.font = CreateFontW(-(9 * a.dpi as i32 / 72), 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
        a.font_big = CreateFontW(-(11 * a.dpi as i32 / 72), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());

        let arrow = LoadCursorW(0, 32512 as *const u16);
        let hand = LoadCursorW(0, 32649 as *const u16);
        let big = make_icon(GetSystemMetrics(11)); // SM_CXICON (barra delle applicazioni / Alt+Tab)
        for (name, proc_, cursor, bg) in [
            ("msQuick", quick_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT, arrow, 16isize),
            ("msConfig", config_proc, arrow, 16),
            ("msHotkey", hkbox_proc, hand, 0),
        ] {
            let cls = wide(name);
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32, style: 0, lpfnWndProc: Some(proc_), cbClsExtra: 0,
                cbWndExtra: 0, hInstance: a.hinst, hIcon: big, hCursor: cursor, hbrBackground: bg,
                lpszMenuName: null(), lpszClassName: cls.as_ptr(), hIconSm: a.icon,
            };
            RegisterClassExW(&wc);
        }
    }
    a.ui_ready = true;
}

#[allow(clippy::too_many_arguments)]
fn ctl(parent: HWND, class: &str, text: &str, style: u32, ex: u32, x: i32, y: i32, w: i32, h: i32, id: isize, font: isize) -> HWND {
    unsafe {
        let hw = CreateWindowExW(ex, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | WS_VISIBLE | style,
                                 s(x), s(y), s(w), s(h), parent, id, app().hinst, null_mut());
        SendMessageW(hw, WM_SETFONT, font as usize, 1);
        hw
    }
}

fn get_text(h: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn create_frame(class: &str, title: &str, ex: u32, cw: i32, ch: i32, place: impl Fn(i32, i32) -> (i32, i32)) -> HWND {
    let style = WS_CAPTION | WS_SYSMENU;
    let mut rc = RECT { left: 0, top: 0, right: s(cw), bottom: s(ch) };
    unsafe {
        AdjustWindowRectEx(&mut rc, style, 0, ex);
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let (x, y) = place(w, h);
        CreateWindowExW(ex, wide(class).as_ptr(), wide(title).as_ptr(), style, x, y, w, h, 0, 0, app().hinst, null_mut())
    }
}

/// Area di lavoro del monitor secondario (non primario), o del primario se ce n'e' uno solo.
fn work_area(secondary: bool) -> RECT {
    let mut primary = RECT::default();
    let mut other: Option<RECT> = None;
    for hm in hmonitors() {
        let mut mi: MONITORINFOEXW = unsafe { zeroed() };
        mi.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(hm, &mut mi) } == 0 { continue; }
        if mi.dwFlags & 1 != 0 { primary = mi.rcWork; } else if other.is_none() { other = Some(mi.rcWork); }
    }
    if secondary { other.unwrap_or(primary) } else { primary }
}

// ───── Finestra rapida ─────

fn show_quick() {
    let a = app();
    if a.quick != 0 { unsafe { SetForegroundWindow(a.quick) }; return; }
    ensure_ui();
    let wa = work_area(true);
    let hwnd = create_frame("msQuick", "Switch Monitor", WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_DLGMODALFRAME, 320, 170,
        |w, h| (wa.left + (wa.right - wa.left - w) / 2, wa.top + (wa.bottom - wa.top - h) / 2));
    if hwnd == 0 { return; }
    a.quick = hwnd;
    a.quick_status = ctl(hwnd, "STATIC", "", SS_CENTER | SS_CENTERIMAGE, 0, 10, 12, 300, 26, 0, a.font);
    let b1 = ctl(hwnd, "BUTTON", "PC\n(DisplayPort)", BS_MULTILINE | WS_TABSTOP, 0, 14, 50, 140, 80, 201, a.font_big);
    ctl(hwnd, "BUTTON", "PS5\n(HDMI)", BS_MULTILINE | WS_TABSTOP, 0, 166, 50, 140, 80, 202, a.font_big);
    update_quick_status();
    unsafe {
        ShowWindow(hwnd, 5);
        SetForegroundWindow(hwnd);
        SetFocus(b1);
    }
}

fn update_quick_status() {
    let a = app();
    if a.quick_status != 0 {
        unsafe { SetWindowTextW(a.quick_status, wide(&format!("Ingresso attuale: {}", current_label())).as_ptr()) };
    }
}

unsafe extern "system" fn quick_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            match wp & 0xFFFF {
                id @ (201 | 202) => {
                    let st = app().quick_status;
                    SetWindowTextW(st, wide("Cambio ingresso...").as_ptr());
                    UpdateWindow(st);
                    switch_to(id == 202);
                    SetTimer(h, 1, 1200, null());
                }
                IDCANCEL => { DestroyWindow(h); }
                _ => {}
            }
            0
        }
        WM_TIMER => { KillTimer(h, 1); update_quick_status(); 0 }
        WM_CLOSE => { DestroyWindow(h); 0 }
        WM_DESTROY => { let a = app(); a.quick = 0; a.quick_status = 0; 0 }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

// ───── Configurazione ─────

fn show_config() {
    let a = app();
    if a.config != 0 { unsafe { SetForegroundWindow(a.config) }; return; }
    ensure_ui();
    // Libera gli hotkey mentre la finestra e' aperta: altrimenti Windows li intercetta
    // e il registratore non riceverebbe la combinazione. Si ri-registrano alla chiusura.
    for id in 1..=4 { unsafe { UnregisterHotKey(a.main, id) }; }

    let wa = work_area(false);
    let hwnd = create_frame("msConfig", "Swytch - Configurazione", 0, 400, 410,
        |w, h| (wa.left + (wa.right - wa.left - w) / 2, wa.top + (wa.bottom - wa.top - h) / 2));
    if hwnd == 0 { apply_hotkeys(); return; }
    a.config = hwnd;
    let f = a.font;
    let cfg = a.cfg.clone();

    ctl(hwnd, "STATIC", "Monitor da controllare:", 0, 0, 14, 14, 372, 20, 0, f);
    let combo = ctl(hwnd, "COMBOBOX", "", CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP, 0, 14, 36, 372, 200, 10, f);
    unsafe {
        SendMessageW(combo, CB_ADDSTRING, 0, wide("Automatico (primo monitor compatibile)").as_ptr() as LPARAM);
        let list = phys_monitors();
        for (i, p) in list.iter().enumerate() {
            let ok = vcp_get(p.h).is_some();
            let t = format!("#{}  {}{}", i, p.name, if ok { "" } else { "  (no DDC/CI)" });
            SendMessageW(combo, CB_ADDSTRING, 0, wide(&t).as_ptr() as LPARAM);
        }
        let count = list.len() as i32;
        release(&list);
        let sel = if cfg.mon >= 0 && cfg.mon < count { cfg.mon + 1 } else { 0 };
        SendMessageW(combo, CB_SETCURSEL, sel as usize, 0);
    }

    ctl(hwnd, "STATIC", "Codice DisplayPort / PC:", 0, 0, 14, 73, 170, 20, 0, f);
    let dp = ctl(hwnd, "EDIT", &format!("0x{:02X}", cfg.dp), ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, 190, 70, 80, 23, 11, f);
    ctl(hwnd, "STATIC", "Codice HDMI / PS5:", 0, 0, 14, 101, 170, 20, 0, f);
    let hdmi = ctl(hwnd, "EDIT", &format!("0x{:02X}", cfg.hdmi), ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, 190, 98, 80, 23, 12, f);

    ctl(hwnd, "BUTTON", "Hotkey (clicca un campo e premi la combinazione)", BS_GROUPBOX, 0, 14, 134, 372, 190, 0, f);
    let chk_hk = ctl(hwnd, "BUTTON", "Abilita hotkey globali", BS_AUTOCHECKBOX | WS_TABSTOP, 0, 26, 156, 350, 20, 13, f);
    let labels = ["PC — DisplayPort:", "PS5 — HDMI:", "Alterna:", "Finestra rapida:"];
    let mut boxes = [0isize; 4];
    for i in 0..4 {
        let y = 184 + i as i32 * 28;
        ctl(hwnd, "STATIC", labels[i], 0, 0, 26, y + 3, 130, 20, 0, f);
        boxes[i] = ctl(hwnd, "msHotkey", "", WS_TABSTOP, WS_EX_CLIENTEDGE, 160, y, 214, 23, 20 + i as isize, f);
        unsafe { SetWindowLongPtrW(boxes[i], GWLP_USERDATA, cfg.hk[i].pack()) };
    }
    ctl(hwnd, "STATIC", "Canc/Backspace = nessun tasto.  Tab = campo successivo.", 0, 0, 26, 298, 350, 20, 0, f);

    let chk_auto = ctl(hwnd, "BUTTON", "Avvia automaticamente al login (immediato)", BS_AUTOCHECKBOX | WS_TABSTOP, 0, 14, 334, 372, 20, 14, f);
    ctl(hwnd, "BUTTON", "Rileva codici", WS_TABSTOP, 0, 14, 368, 110, 28, 101, f);
    ctl(hwnd, "BUTTON", "Default", WS_TABSTOP, 0, 130, 368, 80, 28, 102, f);
    ctl(hwnd, "BUTTON", "OK", BS_DEFPUSHBUTTON | WS_TABSTOP, 0, 220, 368, 80, 28, IDOK as isize, f);
    ctl(hwnd, "BUTTON", "Annulla", WS_TABSTOP, 0, 306, 368, 80, 28, IDCANCEL as isize, f);

    unsafe {
        SendMessageW(chk_hk, BM_SETCHECK, cfg.hotkeys as usize, 0);
        SendMessageW(chk_auto, BM_SETCHECK, cfg.autostart as usize, 0);
        a.ui = Some(CfgUi { combo, dp, hdmi, chk_hk, chk_auto, boxes });
        ShowWindow(hwnd, 5);
        SetForegroundWindow(hwnd);
        SetFocus(combo);
    }
}

fn config_ok(h: HWND) {
    let a = app();
    let Some(ui) = a.ui else { return };
    unsafe {
        let hks: Vec<Hotkey> = ui.boxes.iter().map(|&b| Hotkey::unpack(GetWindowLongPtrW(b, GWLP_USERDATA))).collect();
        for i in 0..4 {
            for j in i + 1..4 {
                if hks[i].is_set() && hks[i] == hks[j] {
                    MessageBoxW(h, wide(&format!("La combinazione {} e' assegnata a due azioni ({} e {}).",
                                                  hks[i].label(), HOTKEY_NAMES[i], HOTKEY_NAMES[j])).as_ptr(),
                                wide("Hotkey duplicato").as_ptr(), 0x30);
                    return;
                }
            }
        }
        let mut c = a.cfg.clone();
        let sel = SendMessageW(ui.combo, CB_GETCURSEL, 0, 0) as i32;
        c.mon = if sel <= 0 { -1 } else { sel - 1 };
        let d = Config::defaults();
        c.dp = parse_num(&get_text(ui.dp)).map(|v| v as u32).unwrap_or(d.dp);
        c.hdmi = parse_num(&get_text(ui.hdmi)).map(|v| v as u32).unwrap_or(d.hdmi);
        c.hotkeys = SendMessageW(ui.chk_hk, BM_GETCHECK, 0, 0) == 1;
        c.autostart = SendMessageW(ui.chk_auto, BM_GETCHECK, 0, 0) == 1;
        c.hk = [hks[0], hks[1], hks[2], hks[3]];
        let auto_changed = c.autostart != a.cfg.autostart;
        a.cfg = c;
        a.cfg.save();
        if auto_changed {
            let ok = if a.cfg.autostart {
                std::env::current_exe().map(|e| install_task(&e)).unwrap_or(false)
            } else { uninstall_task() };
            if !ok {
                MessageBoxW(h, wide("Impossibile aggiornare l'avvio automatico (attivita' pianificata).").as_ptr(),
                            wide("Swytch").as_ptr(), 0x30);
            }
        }
        DestroyWindow(h); // WM_DESTROY ri-registra gli hotkey con la nuova configurazione
    }
}

unsafe extern "system" fn config_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            match wp & 0xFFFF {
                IDOK => config_ok(h),
                IDCANCEL => { DestroyWindow(h); }
                101 => { MessageBoxW(h, wide(&ddc_report()).as_ptr(), wide("Monitor rilevati").as_ptr(), 0x40); }
                102 => {
                    if let Some(ui) = app().ui {
                        let d = Config::defaults();
                        for i in 0..4 {
                            SetWindowLongPtrW(ui.boxes[i], GWLP_USERDATA, d.hk[i].pack());
                            InvalidateRect(ui.boxes[i], null(), 1);
                        }
                    }
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => { DestroyWindow(h); 0 }
        WM_DESTROY => {
            let a = app();
            a.config = 0;
            a.ui = None;
            apply_hotkeys();
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

// ───── Registratore di hotkey (controllo personalizzato) ─────

fn hk_record(h: HWND, vk: u32) {
    unsafe {
        // Solo modificatori: aspetta il tasto vero.
        if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5) { return; }
        let down = |k: i32| GetKeyState(k) < 0;
        let mut m = 0u32;
        if down(0x11) { m |= MOD_CONTROL; }
        if down(0x12) { m |= MOD_ALT; }
        if down(0x10) { m |= MOD_SHIFT; }
        if down(0x5B) || down(0x5C) { m |= MOD_WIN; }

        let parent = GetParent(h);
        if vk == 0x09 && (m == 0 || m == MOD_SHIFT) {
            // Tab / Shift+Tab: navigazione normale tra i campi.
            SetFocus(GetNextDlgTabItem(parent, h, (m == MOD_SHIFT) as BOOL));
            return;
        }
        if m == 0 {
            match vk {
                0x1B => { PostMessageW(parent, WM_COMMAND, IDCANCEL, 0); return; } // Esc = Annulla
                0x0D => return,                                                    // Invio: niente
                0x2E | 0x08 => {                                                   // Canc/Backspace = nessuno
                    SetWindowLongPtrW(h, GWLP_USERDATA, Hotkey::NONE.pack() | REC_FLAG);
                    InvalidateRect(h, null(), 1);
                    return;
                }
                _ => {}
            }
        }
        SetWindowLongPtrW(h, GWLP_USERDATA, Hotkey { m, vk }.pack() | REC_FLAG);
        InvalidateRect(h, null(), 1);
    }
}

unsafe extern "system" fn hkbox_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_GETDLGCODE => DLGC_WANTALLKEYS,
        WM_LBUTTONDOWN => { SetFocus(h); 0 }
        WM_SETFOCUS => {
            let v = GetWindowLongPtrW(h, GWLP_USERDATA) & !REC_FLAG;
            SetWindowLongPtrW(h, GWLP_USERDATA, v);
            InvalidateRect(h, null(), 1);
            0
        }
        WM_KILLFOCUS => { InvalidateRect(h, null(), 1); 0 }
        WM_KEYDOWN | WM_SYSKEYDOWN => { hk_record(h, wp as u32); 0 }
        // Non passare nulla a DefWindowProc: niente menu di sistema con Alt/F10, niente "beep".
        WM_CHAR | WM_SYSCHAR | WM_KEYUP | WM_SYSKEYUP => 0,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(h, &mut ps);
            let mut rc = RECT::default();
            GetClientRect(h, &mut rc);
            let focused = GetFocus() == h;
            let v = GetWindowLongPtrW(h, GWLP_USERDATA);
            if focused {
                let br = CreateSolidBrush(0x00D7_FAFF); // RGB(255,250,215) in formato 0x00BBGGRR
                FillRect(dc, &rc, br);
                DeleteObject(br);
            } else {
                FillRect(dc, &rc, GetSysColorBrush(5)); // COLOR_WINDOW
            }
            let old = SelectObject(dc, app().font);
            SetBkMode(dc, 1); // TRANSPARENT
            SetTextColor(dc, GetSysColor(8)); // COLOR_WINDOWTEXT
            let text = if focused && v & REC_FLAG == 0 { "Premi la combinazione...".to_string() } else { Hotkey::unpack(v).label() };
            rc.left += s(6);
            DrawTextW(dc, wide(&text).as_ptr(), -1, &mut rc, 0x20 | 0x4 | 0x800); // SINGLELINE|VCENTER|NOPREFIX
            SelectObject(dc, old);
            EndPaint(h, &ps);
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}
