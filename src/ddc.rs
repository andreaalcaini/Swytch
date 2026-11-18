//! Monitor input switching over DDC/CI: VCP feature 0x60 (Input Select) through Dxva2.
//!
//! Physical monitors are enumerated fresh for every operation (the user can plug and unplug screens
//! at any time) and released when the [`Monitors`] list is dropped. "Auto" picks the first monitor
//! that answers a VCP 0x60 read; monitors without DDC/CI (or behind adapters that drop it) don't.

use std::mem::{size_of, zeroed};
use std::ptr::null;

use crate::i18n::{fmt, Strings};
use crate::win32::*;

const VCP_INPUT: u8 = 0x60;

/// Why a switch failed; the UI maps it to its own text.
#[derive(Debug, PartialEq)]
pub enum DdcError {
    NoMonitor,
    Rejected { monitor: String },
}

impl DdcError {
    pub fn message(&self, s: &Strings) -> String {
        match self {
            DdcError::NoMonitor => s.err_no_monitor.into(),
            DdcError::Rejected { monitor } => fmt(s.err_rejected, &[monitor]),
        }
    }
}

pub struct Phys {
    pub h: isize,
    pub name: String,
}

/// The physical monitors of every display, released on drop.
pub struct Monitors(pub Vec<Phys>);

impl Drop for Monitors {
    fn drop(&mut self) {
        for p in &self.0 {
            // SAFETY: each handle came from GetPhysicalMonitorsFromHMONITOR and is released once.
            unsafe { DestroyPhysicalMonitor(p.h) };
        }
    }
}

unsafe extern "system" fn enum_mon_cb(h: isize, _dc: isize, _rc: *mut RECT, l: LPARAM) -> BOOL {
    (*(l as *mut Vec<isize>)).push(h);
    1
}

/// Every display (`HMONITOR`).
pub fn hmonitors() -> Vec<isize> {
    let mut v: Vec<isize> = Vec::new();
    // SAFETY: the callback only runs during the call and only pushes into `v`.
    unsafe { EnumDisplayMonitors(0, null(), Some(enum_mon_cb), &mut v as *mut Vec<isize> as LPARAM) };
    v
}

/// Manufacturer of a PnP vendor ID (the first three letters of the EDID ID, e.g. `LEN67C5`).
pub fn vendor(pnp: &str) -> &'static str {
    match pnp {
        "LEN" => "Lenovo",
        "SAM" | "SEC" => "Samsung",
        "DEL" => "Dell",
        "GSM" => "LG",
        "ACI" | "AUS" => "Asus",
        "BNQ" => "BenQ",
        "AOC" => "AOC",
        "HWP" | "HPN" => "HP",
        "PHL" => "Philips",
        "GBT" => "Gigabyte",
        "MSI" => "MSI",
        "ACR" => "Acer",
        "VSC" => "ViewSonic",
        "IVM" => "Iiyama",
        "EIZ" | "ENC" => "Eizo",
        "SNY" => "Sony",
        "AUO" => "AUO",
        "BOE" => "BOE",
        "XMI" => "Xiaomi",
        "HKC" => "HKC",
        _ => "",
    }
}

/// Readable name from a monitor device ID (`MONITOR\LEN67C5\{...}\0001` → `Lenovo (LEN67C5)`).
pub fn name_from_device_id(id: &str) -> Option<String> {
    let code = id.split('\\').nth(1)?;
    if code.len() < 3 || !code.is_char_boundary(3) {
        return None;
    }
    Some(match vendor(&code[..3]) {
        "" => code.to_string(),
        v => format!("{v} ({code})"),
    })
}

fn monitor_name(hmon: isize) -> String {
    // SAFETY: both structs are zeroed with their size set, as the API requires.
    unsafe {
        let mut mi: MONITORINFOEXW = zeroed();
        mi.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut mi) == 0 {
            return "Monitor".into();
        }
        let mut dd: DISPLAY_DEVICEW = zeroed();
        dd.cb = size_of::<DISPLAY_DEVICEW>() as u32;
        if EnumDisplayDevicesW(mi.szDevice.as_ptr(), 0, &mut dd, 0) == 0 {
            return from_wide(&mi.szDevice);
        }
        name_from_device_id(&from_wide(&dd.DeviceID)).unwrap_or_else(|| from_wide(&dd.DeviceString))
    }
}

pub fn monitors() -> Monitors {
    let mut out = Vec::new();
    for hm in hmonitors() {
        // SAFETY: the array is sized from GetNumberOfPhysicalMonitorsFromHMONITOR.
        unsafe {
            let mut n = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hm, &mut n) == 0 || n == 0 {
                continue;
            }
            let empty = PHYSICAL_MONITOR { hPhysicalMonitor: 0, szPhysicalMonitorDescription: [0; 128] };
            let mut arr = vec![empty; n as usize];
            if GetPhysicalMonitorsFromHMONITOR(hm, n, arr.as_mut_ptr()) == 0 {
                continue;
            }
            let name = monitor_name(hm);
            out.extend(arr.into_iter().map(|p| Phys { h: p.hPhysicalMonitor, name: name.clone() }));
        }
    }
    Monitors(out)
}

/// Current input of a physical monitor, `None` if it doesn't answer.
pub fn vcp_input(h: isize) -> Option<u32> {
    let (mut t, mut cur, mut max) = (0u32, 0u32, 0u32);
    // SAFETY: valid handle and out pointers.
    let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(h, VCP_INPUT, &mut t, &mut cur, &mut max) } != 0;
    ok.then_some(cur)
}

/// The monitor to drive: index `idx`, or (auto, or an index that no longer exists) the first that answers.
fn pick(list: &[Phys], idx: i32) -> Option<&Phys> {
    if let Some(p) = usize::try_from(idx).ok().and_then(|i| list.get(i)) {
        return Some(p);
    }
    list.iter().find(|p| vcp_input(p.h).is_some())
}

/// Switches the input; on success returns the name of the monitor that accepted it.
pub fn set_input(idx: i32, value: u32) -> Result<String, DdcError> {
    let list = monitors();
    let p = pick(&list.0, idx).ok_or(DdcError::NoMonitor)?;
    // SAFETY: valid handle.
    if unsafe { SetVCPFeature(p.h, VCP_INPUT, value) } != 0 {
        Ok(p.name.clone())
    } else {
        Err(DdcError::Rejected { monitor: p.name.clone() })
    }
}

pub fn get_input(idx: i32) -> Option<u32> {
    let list = monitors();
    pick(&list.0, idx).and_then(|p| vcp_input(p.h))
}

/// Input codes listed for VCP 0x60 in an MCCS capabilities string (`… 60(11 12 0F) …` → `11 12 0F`).
pub fn input_codes(caps: &str) -> Option<String> {
    let b = caps.as_bytes();
    let mut from = 0;
    while let Some(pos) = caps[from..].find("60(") {
        let at = from + pos;
        // A real code starts a token: "B60(" or "260(" are something else.
        if at == 0 || b[at - 1] == b' ' || b[at - 1] == b'(' {
            let rest = &caps[at + 3..];
            return rest.find(')').map(|e| rest[..e].trim().to_string());
        }
        from = at + 3;
    }
    None
}

fn capabilities(h: isize) -> Option<String> {
    // SAFETY: the buffer is sized from GetCapabilitiesStringLength.
    unsafe {
        let mut len = 0u32;
        if GetCapabilitiesStringLength(h, &mut len) == 0 || len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        if CapabilitiesRequestAndCapabilitiesReply(h, buf.as_mut_ptr(), len) == 0 {
            return None;
        }
        Some(String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string())
    }
}

/// Text for "Detect codes": every monitor, its current input and the input codes it lists.
pub fn report(s: &Strings) -> String {
    let list = monitors();
    let mut out = String::new();
    for (i, p) in list.0.iter().enumerate() {
        out.push_str(&fmt(s.report_monitor, &[&i, &p.name]));
        out.push('\n');
        match vcp_input(p.h) {
            Some(v) => out.push_str(&fmt(s.report_input, &[&format!("{v:02X}")])),
            None => out.push_str(s.report_no_input),
        }
        out.push('\n');
        if let Some(c) = capabilities(p.h).as_deref().and_then(input_codes) {
            out.push_str(&fmt(s.report_codes, &[&c]));
            out.push('\n');
        }
        out.push('\n');
    }
    if out.is_empty() {
        out = s.report_none.into();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENOVO_CAPS: &str = include_str!("../testdata/lenovo-r27qe-capabilities.txt");

    #[test]
    fn input_codes_from_real_capabilities() {
        assert_eq!(input_codes(LENOVO_CAPS).as_deref(), Some("11 12 0F"));
    }

    #[test]
    fn input_codes_ignore_other_features() {
        assert_eq!(input_codes("vcp(02 B60(01) 260(02) 14(05))"), None);
        assert_eq!(input_codes("vcp(60( 0F 11 ) 62)").as_deref(), Some("0F 11"));
        assert_eq!(input_codes("60(0F"), None);
    }

    #[test]
    fn names_from_device_ids() {
        assert_eq!(name_from_device_id(r"MONITOR\LEN67C5\{4d36e96e}\0001").as_deref(), Some("Lenovo (LEN67C5)"));
        assert_eq!(name_from_device_id(r"MONITOR\ZZZ1234\{x}\0002").as_deref(), Some("ZZZ1234"));
        assert_eq!(name_from_device_id("garbage"), None);
    }

    #[test]
    fn error_messages() {
        let e = DdcError::Rejected { monitor: "Lenovo (LEN67C5)".into() };
        assert_eq!(e.message(&crate::i18n::EN), "Lenovo (LEN67C5) did not accept the command");
    }
}
