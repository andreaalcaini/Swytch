//! UI strings in English (the default and the fallback) and Italian.
//!
//! Both language packs are values of the same struct, so a key missing in either one is a compile
//! error. Placeholders are `{0}`, `{1}`… and filled with [`fmt`]; a test checks that both languages
//! use the same placeholders for every key. The language comes from the config (`Language=`) or,
//! when that is `auto`, from the Windows display language.

use std::fmt::Display;
use std::sync::OnceLock;

use crate::win32::GetUserDefaultUILanguage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    It,
}

impl Lang {
    /// Value of the `Language=` config key: `en`, `it`, anything else means "follow Windows".
    pub fn from_setting(s: &str) -> Lang {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" => Lang::En,
            "it" => Lang::It,
            _ => Lang::detect(),
        }
    }

    pub fn detect() -> Lang {
        // SAFETY: no arguments, no side effects.
        let id = unsafe { GetUserDefaultUILanguage() };
        if id & 0x3FF == 0x10 {
            Lang::It // LANG_ITALIAN, any sublanguage
        } else {
            Lang::En
        }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::En => &EN,
            Lang::It => &IT,
        }
    }
}

static CURRENT: OnceLock<&'static Strings> = OnceLock::new();

/// Picks the language once at startup; later calls are ignored.
pub fn init(lang: Lang) {
    let _ = CURRENT.set(lang.strings());
}

/// The strings of the current language.
pub fn tr() -> &'static Strings {
    CURRENT.get_or_init(|| Lang::detect().strings())
}

/// Replaces `{0}`, `{1}`… in `template` with `args`.
pub fn fmt(template: &str, args: &[&dyn Display]) -> String {
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &a.to_string());
    }
    out
}

macro_rules! strings {
    ($($key:ident),* $(,)?) => {
        pub struct Strings {
            $(pub $key: &'static str,)*
        }

        impl Strings {
            /// Every key with its text (used by the tests).
            #[cfg(test)]
            pub fn entries(&self) -> Vec<(&'static str, &'static str)> {
                vec![$((stringify!($key), self.$key),)*]
            }
        }
    };
}

strings! {
    // Actions: tray menu entries, settings labels, duplicate-hotkey message.
    action_dp, action_hdmi, action_toggle, action_quick,
    menu_settings, menu_exit,
    // Inputs and current state.
    input_dp, input_hdmi, state_pc, state_ps5, state_unknown, state_unreadable,
    tip_last,
    // Notifications.
    hotkeys_failed, switch_failed, err_no_monitor, err_rejected,
    // Quick switch window.
    quick_title, quick_current, quick_switching, quick_pc, quick_ps5,
    // Settings window.
    settings_title, monitor_label, monitor_auto, monitor_no_ddc, dp_code_label, hdmi_code_label,
    hotkeys_group, hotkeys_enable, hotkeys_hint, hotkey_press, hotkey_none, autostart_label,
    btn_detect, btn_defaults, btn_ok, btn_cancel,
    duplicate_title, duplicate_text, autostart_failed,
    // "Detect codes" report.
    report_title, report_monitor, report_input, report_no_input, report_codes, report_none,
    // Key names that differ between languages.
    key_enter, key_space, key_pgup, key_pgdn, key_end, key_left, key_up, key_right, key_down,
    key_prtsc, key_del, key_pause, key_scrlk, key_mute, key_next, key_prev,
}

impl Strings {
    pub fn actions(&self) -> [&'static str; 4] {
        [self.action_dp, self.action_hdmi, self.action_toggle, self.action_quick]
    }
}

pub const EN: Strings = Strings {
    action_dp: "PC — DisplayPort",
    action_hdmi: "PS5 — HDMI",
    action_toggle: "Toggle",
    action_quick: "Quick switch",
    menu_settings: "Settings...",
    menu_exit: "Exit",
    input_dp: "DisplayPort (PC)",
    input_hdmi: "HDMI (PS5)",
    state_pc: "PC (DisplayPort)",
    state_ps5: "PS5 (HDMI)",
    state_unknown: "unknown (0x{0})",
    state_unreadable: "not readable",
    tip_last: "Swytch {0} - last: {1}",
    hotkeys_failed: "Some hotkeys could not be registered (already used by another app?)",
    switch_failed: "Switch to {0} failed: {1}",
    err_no_monitor: "no DDC/CI capable monitor",
    err_rejected: "{0} did not accept the command",
    quick_title: "Switch Monitor",
    quick_current: "Current input: {0}",
    quick_switching: "Switching input...",
    quick_pc: "PC\n(DisplayPort)",
    quick_ps5: "PS5\n(HDMI)",
    settings_title: "Swytch {0} - Settings",
    monitor_label: "Monitor to control:",
    monitor_auto: "Automatic (first compatible monitor)",
    monitor_no_ddc: "  (no DDC/CI)",
    dp_code_label: "DisplayPort / PC code:",
    hdmi_code_label: "HDMI / PS5 code:",
    hotkeys_group: "Hotkeys (click a field and press the combination)",
    hotkeys_enable: "Enable global hotkeys",
    hotkeys_hint: "Del/Backspace = no key.  Tab = next field.",
    hotkey_press: "Press the combination...",
    hotkey_none: "(none)",
    autostart_label: "Start automatically at sign-in (immediately)",
    btn_detect: "Detect codes",
    btn_defaults: "Defaults",
    btn_ok: "OK",
    btn_cancel: "Cancel",
    duplicate_title: "Duplicate hotkey",
    duplicate_text: "The combination {0} is assigned to two actions ({1} and {2}).",
    autostart_failed: "Could not update the start at sign-in (scheduled task).",
    report_title: "Detected monitors",
    report_monitor: "Monitor #{0}: {1}",
    report_input: "    current input = 0x{0}",
    report_no_input: "    input: not supported (no DDC/CI)",
    report_codes: "    supported input codes (60): {0}",
    report_none: "No monitor detected.",
    key_enter: "Enter",
    key_space: "Space",
    key_pgup: "PgUp",
    key_pgdn: "PgDn",
    key_end: "End",
    key_left: "Left",
    key_up: "Up",
    key_right: "Right",
    key_down: "Down",
    key_prtsc: "PrtSc",
    key_del: "Del",
    key_pause: "Pause",
    key_scrlk: "ScrLk",
    key_mute: "Mute",
    key_next: "Next",
    key_prev: "Prev",
};

pub const IT: Strings = Strings {
    action_dp: "PC — DisplayPort",
    action_hdmi: "PS5 — HDMI",
    action_toggle: "Alterna",
    action_quick: "Finestra rapida",
    menu_settings: "Configura...",
    menu_exit: "Esci",
    input_dp: "DisplayPort (PC)",
    input_hdmi: "HDMI (PS5)",
    state_pc: "PC (DisplayPort)",
    state_ps5: "PS5 (HDMI)",
    state_unknown: "sconosciuto (0x{0})",
    state_unreadable: "non leggibile",
    tip_last: "Swytch {0} - ultimo: {1}",
    hotkeys_failed: "Alcuni hotkey non sono stati registrati (già in uso da un'altra app?)",
    switch_failed: "Switch a {0} fallito: {1}",
    err_no_monitor: "nessun monitor compatibile DDC/CI",
    err_rejected: "{0} non ha accettato il comando",
    quick_title: "Switch Monitor",
    quick_current: "Ingresso attuale: {0}",
    quick_switching: "Cambio ingresso...",
    quick_pc: "PC\n(DisplayPort)",
    quick_ps5: "PS5\n(HDMI)",
    settings_title: "Swytch {0} - Configurazione",
    monitor_label: "Monitor da controllare:",
    monitor_auto: "Automatico (primo monitor compatibile)",
    monitor_no_ddc: "  (no DDC/CI)",
    dp_code_label: "Codice DisplayPort / PC:",
    hdmi_code_label: "Codice HDMI / PS5:",
    hotkeys_group: "Hotkey (clicca un campo e premi la combinazione)",
    hotkeys_enable: "Abilita hotkey globali",
    hotkeys_hint: "Canc/Backspace = nessun tasto.  Tab = campo successivo.",
    hotkey_press: "Premi la combinazione...",
    hotkey_none: "(nessuno)",
    autostart_label: "Avvia automaticamente al login (immediato)",
    btn_detect: "Rileva codici",
    btn_defaults: "Default",
    btn_ok: "OK",
    btn_cancel: "Annulla",
    duplicate_title: "Hotkey duplicato",
    duplicate_text: "La combinazione {0} è assegnata a due azioni ({1} e {2}).",
    autostart_failed: "Impossibile aggiornare l'avvio automatico (attività pianificata).",
    report_title: "Monitor rilevati",
    report_monitor: "Monitor #{0}: {1}",
    report_input: "    ingresso attuale = 0x{0}",
    report_no_input: "    ingresso: non supportato (DDC/CI assente)",
    report_codes: "    codici ingresso supportati (60): {0}",
    report_none: "Nessun monitor rilevato.",
    key_enter: "Invio",
    key_space: "Spazio",
    key_pgup: "PagSu",
    key_pgdn: "PagGiù",
    key_end: "Fine",
    key_left: "Sinistra",
    key_up: "Su",
    key_right: "Destra",
    key_down: "Giù",
    key_prtsc: "Stamp",
    key_del: "Canc",
    key_pause: "Pausa",
    key_scrlk: "BlocScorr",
    key_mute: "Muto",
    key_next: "Succ",
    key_prev: "Prec",
};

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> Vec<String> {
        let mut v: Vec<String> = (0..10).map(|i| format!("{{{i}}}")).filter(|p| s.contains(p.as_str())).collect();
        v.sort();
        v
    }

    #[test]
    fn languages_use_the_same_placeholders() {
        for ((key, en), (_, it)) in EN.entries().into_iter().zip(IT.entries()) {
            assert_eq!(placeholders(en), placeholders(it), "placeholder mismatch for `{key}`");
            assert!(!en.is_empty() && !it.is_empty(), "empty text for `{key}`");
        }
    }

    #[test]
    fn fmt_fills_numbered_placeholders() {
        assert_eq!(fmt("{1} and {0}", &[&"a", &2]), "2 and a");
        assert_eq!(fmt("no placeholders", &[&"x"]), "no placeholders");
    }

    #[test]
    fn setting_overrides_detection() {
        assert_eq!(Lang::from_setting("it"), Lang::It);
        assert_eq!(Lang::from_setting(" EN "), Lang::En);
    }
}
