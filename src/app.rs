//! The running app: state, the hidden main window, hotkeys and the switch actions.
//!
//! Startup order is what makes the app usable right after sign-in: the hotkeys are registered
//! first (they work before Explorer is up), then the tray icon is added, retrying every 250 ms
//! until the taskbar exists and again whenever Explorer restarts ("TaskbarCreated"). Everything
//! the windows need (fonts, classes, visual styles) is created only when one is first opened.
//!
//! All state lives in one [`App`] owned by the UI thread; window procedures reach it through
//! [`app()`]. There is only that one thread, so there is no locking.

use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::time::Instant;

use crate::config::Config;
use crate::ddc;
use crate::i18n::{self, fmt, tr, Lang};
use crate::logging::{log, ms_since_process_start};
use crate::tray;
use crate::trayicon::make_icon;
use crate::ui::{self, settings::SettingsUi};
use crate::win32::*;
use crate::VERSION;

/// Class of the hidden main window; `install` finds a running instance by it.
pub const MAIN_CLASS: &str = "SwytchMain";
const MUTEX_NAME: &str = "Swytch_SingleInstance";

pub const WM_TRAY: u32 = WM_APP + 1;
const TIMER_TRAY: usize = 1;

// Commands of the tray menu, also accepted as WM_COMMAND by the main window.
pub const CMD_DP: usize = 1;
pub const CMD_HDMI: usize = 2;
pub const CMD_TOGGLE: usize = 3;
pub const CMD_QUICK: usize = 4;
pub const CMD_SETTINGS: usize = 5;
pub const CMD_EXIT: usize = 6;

/// Hotkey ids passed to RegisterHotKey, in config order (DP, HDMI, toggle, quick window).
const HOTKEY_IDS: [i32; 4] = [1, 2, 3, 4];
/// Names used in the log (not shown in the UI).
const HOTKEY_LOG_NAMES: [&str; 4] = ["DP", "HDMI", "Toggle", "Window"];

pub struct App {
    pub hinst: isize,
    pub main: HWND,
    pub cfg: Config,
    pub icon: isize,
    pub tray: bool,
    pub wm_taskbar: u32,
    pub dpi: u32,
    pub ui_ready: bool,
    pub font: isize,
    pub font_big: isize,
    pub quick: HWND,
    pub quick_status: HWND,
    pub settings: HWND,
    pub settings_ui: Option<SettingsUi>,
}

static mut APP: *mut App = null_mut();

/// The app state. Only valid on the UI thread after [`run`] has created it.
#[allow(clippy::mut_from_ref)]
pub fn app() -> &'static mut App {
    // SAFETY: APP is set once in `run` before any window exists, never freed, and only touched
    // from the UI thread (window procedures and the message loop).
    unsafe { &mut *APP }
}

/// Scales a 96-DPI length to the system DPI.
pub fn s(v: i32) -> i32 {
    v * app().dpi as i32 / 96
}

/// Starts the tray app; returns when the user exits.
pub fn run() {
    let t0 = Instant::now();
    // SAFETY: Win32 setup on the UI thread; every pointer passed lives for the duration of its call.
    unsafe {
        // One instance only (same name as the previous version, so they never both run).
        CreateMutexW(null(), 0, wide(MUTEX_NAME).as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
        let hinst = GetModuleHandleW(null());
        let (cfg, existed) = Config::load();
        if !existed {
            cfg.save_logged();
        }
        i18n::init(Lang::from_setting(&cfg.language));

        APP = Box::into_raw(Box::new(App {
            hinst,
            main: 0,
            cfg,
            icon: 0,
            tray: false,
            wm_taskbar: 0,
            dpi: GetDpiForSystem().max(96),
            ui_ready: false,
            font: 0,
            font_big: 0,
            quick: 0,
            quick_status: 0,
            settings: 0,
            settings_ui: None,
        }));

        // Hidden top-level window (not message-only, so it also gets the "TaskbarCreated" broadcast).
        let cls = wide(MAIN_CLASS);
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(main_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: null(),
            lpszClassName: cls.as_ptr(),
            hIconSm: 0,
        };
        RegisterClassExW(&wc);
        let main = CreateWindowExW(0, cls.as_ptr(), wide("Swytch").as_ptr(), 0, 0, 0, 0, 0, 0, 0, hinst, null_mut());
        let a = app();
        a.main = main;

        // 1) Hotkeys first: they work even before Explorer and the taskbar are ready.
        apply_hotkeys();
        log(&format!(
            "Start {VERSION}: hotkeys ready {} ms after process creation (main {} ms, PC up for {:.1} s)",
            ms_since_process_start(),
            t0.elapsed().as_millis(),
            GetTickCount64() as f64 / 1000.0
        ));

        // 2) Tray icon: if the taskbar isn't there yet, retry every 250 ms and on "TaskbarCreated".
        a.wm_taskbar = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        a.icon = make_icon(GetSystemMetrics(SM_CXSMICON));
        if tray::add() {
            log_tray_ready();
        } else {
            SetTimer(main, TIMER_TRAY, 250, null());
        }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            let a = app();
            if a.settings != 0 && IsDialogMessageW(a.settings, &mut msg) != 0 {
                continue;
            }
            if a.quick != 0 && IsDialogMessageW(a.quick, &mut msg) != 0 {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn log_tray_ready() {
    log(&format!("Tray icon added {} ms after process creation", ms_since_process_start()));
}

unsafe extern "system" fn main_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let a = app();
    if a.wm_taskbar != 0 && msg == a.wm_taskbar {
        // Explorer (re)started: the icon must be added again.
        a.tray = false;
        if tray::add() {
            KillTimer(h, TIMER_TRAY);
        } else {
            SetTimer(h, TIMER_TRAY, 250, null());
        }
        return 0;
    }
    match msg {
        WM_HOTKEY => {
            match wp as i32 {
                1 => command(CMD_DP),
                2 => command(CMD_HDMI),
                3 => command(CMD_TOGGLE),
                4 => command(CMD_QUICK),
                _ => {}
            }
            0
        }
        WM_TRAY => {
            let ev = (lp as u32) & 0xFFFF;
            if ev == WM_RBUTTONUP || ev == WM_CONTEXTMENU {
                tray::show_menu();
            } else if ev == WM_LBUTTONDBLCLK {
                command(CMD_QUICK);
            }
            0
        }
        WM_COMMAND => {
            command(wp & 0xFFFF);
            0
        }
        WM_TIMER if wp == TIMER_TRAY => {
            if tray::add() {
                KillTimer(h, TIMER_TRAY);
                log_tray_ready();
            }
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// Runs a tray-menu command (`CMD_*`).
pub fn command(cmd: usize) {
    match cmd {
        CMD_DP => switch_to(false),
        CMD_HDMI => switch_to(true),
        CMD_TOGGLE => toggle(),
        CMD_QUICK => ui::quick::show(),
        CMD_SETTINGS => ui::settings::show(),
        CMD_EXIT => exit(),
        _ => {}
    }
}

pub fn unregister_hotkeys() {
    let main = app().main;
    for id in HOTKEY_IDS {
        // SAFETY: unregistering an id that isn't registered is harmless.
        unsafe { UnregisterHotKey(main, id) };
    }
}

/// (Re)registers the hotkeys from the config, warning in the tray if some are taken.
pub fn apply_hotkeys() {
    unregister_hotkeys();
    let a = app();
    if !a.cfg.hotkeys {
        log("Hotkeys disabled");
        return;
    }
    let mut parts = Vec::new();
    let mut all = true;
    for (i, hk) in a.cfg.hk.iter().enumerate() {
        if !hk.is_set() {
            parts.push(format!("{}=(none)", HOTKEY_LOG_NAMES[i]));
            continue;
        }
        // SAFETY: plain registration on our own window.
        let ok = unsafe { RegisterHotKey(a.main, HOTKEY_IDS[i], hk.m | MOD_NOREPEAT, hk.vk) } != 0;
        all &= ok;
        parts.push(format!("{}[{}]={}", HOTKEY_LOG_NAMES[i], hk.serialize(), if ok { "OK" } else { "FAILED" }));
    }
    log(&format!("Hotkeys: {}", parts.join("  ")));
    if !all {
        tray::balloon(tr().hotkeys_failed);
    }
}

pub fn switch_to(hdmi: bool) {
    let a = app();
    let t = tr();
    let (code, label, log_label) =
        if hdmi { (a.cfg.hdmi, t.input_hdmi, "HDMI") } else { (a.cfg.dp, t.input_dp, "DisplayPort") };
    match ddc::set_input(a.cfg.mon, code) {
        Ok(name) => log(&format!("OK   -> {log_label} (0x{code:02X})  [{name}]")),
        Err(e) => {
            log(&format!("FAIL -> {log_label} (0x{code:02X})  [{e:?}]"));
            tray::balloon(&fmt(t.switch_failed, &[&label, &e.message(t)]));
        }
    }
    tray::set_tip(&fmt(t.tip_last, &[&VERSION, &label]));
}

fn toggle() {
    let a = app();
    let cur = ddc::get_input(a.cfg.mon);
    switch_to(cur == Some(a.cfg.dp));
}

/// The current input as shown in the quick window.
pub fn current_label() -> String {
    let a = app();
    let t = tr();
    match ddc::get_input(a.cfg.mon) {
        Some(v) if v == a.cfg.dp => t.state_pc.into(),
        Some(v) if v == a.cfg.hdmi => t.state_ps5.into(),
        Some(v) => fmt(t.state_unknown, &[&format!("{v:02X}")]),
        None => t.state_unreadable.into(),
    }
}

fn exit() {
    unregister_hotkeys();
    tray::remove();
    log("Exit");
    // SAFETY: ends the message loop of this thread.
    unsafe { PostQuitMessage(0) };
}
