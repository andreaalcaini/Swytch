//! Installation and start at sign-in.
//!
//! Start at sign-in is a scheduled task with a logon trigger, not a `Run` registry value: Windows
//! delays `Run` entries by several seconds after sign-in, while a logon task starts at once, before
//! Explorer is ready. A per-user task with `LeastPrivilege` needs no admin rights. `schtasks /XML`
//! wants the definition as UTF-16 with a BOM.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::ptr::null;

use crate::app::{CMD_EXIT, MAIN_CLASS};
use crate::logging::log;
use crate::paths::{data_dir, install_dir, user_name, APP_NAME, LEGACY_NAME};
use crate::win32::*;

pub const TASK_NAME: &str = "Swytch";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn run_hidden(exe: &str, args: &[&std::ffi::OsStr]) -> bool {
    Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn schtasks(args: &[&str]) -> bool {
    let args: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_ref()).collect();
    run_hidden("schtasks.exe", &args)
}

fn delete_run_value(name: &str) {
    // SAFETY: valid NUL-terminated strings; a missing value is not an error worth reporting.
    unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), wide(name).as_ptr()) };
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x.to_string_lossy().eq_ignore_ascii_case(&y.to_string_lossy()),
        _ => false,
    }
}

/// `Swytch.exe install`: copies the executable to `%LOCALAPPDATA%\Programs\Swytch`, creates the
/// logon task, removes the previous version's start at sign-in and starts the installed app.
/// Running it again with a newer exe is the update path.
pub fn install() -> bool {
    let cur = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log(&format!("Install: cannot locate the running executable: {e}"));
            return false;
        }
    };
    let dir = install_dir();
    let target = dir.join(format!("{APP_NAME}.exe"));
    if !same_file(&cur, &target) {
        stop_running_instance();
        if let Err(e) = fs::create_dir_all(&dir).and_then(|_| fs::copy(&cur, &target)) {
            log(&format!("Install: copying to {target:?} failed: {e}"));
            return false;
        }
    }
    // Previous version ("monitorSwitch"): its task and Run value.
    schtasks(&["/Delete", "/TN", LEGACY_NAME, "/F"]);
    delete_run_value(LEGACY_NAME);

    let ok = install_task(&target);
    if ok {
        schtasks(&["/Run", "/TN", TASK_NAME]);
    }
    log(&format!("Install to {target:?}: {}", if ok { "OK" } else { "FAILED" }));
    ok
}

/// A running instance locks its exe: ask it to quit, and wait up to 2 s.
fn stop_running_instance() {
    let class = wide(MAIN_CLASS);
    // SAFETY: plain window lookups and a posted message.
    unsafe {
        let h = FindWindowW(class.as_ptr(), null());
        if h == 0 {
            return;
        }
        PostMessageW(h, WM_COMMAND, CMD_EXIT, 0);
        for _ in 0..40 {
            if FindWindowW(class.as_ptr(), null()) == 0 {
                break;
            }
            Sleep(50);
        }
    }
}

/// Task Scheduler definition: logon trigger for `user`, no time limit, normal priority.
pub fn task_xml(user: &str, exe: &Path) -> String {
    let workdir = exe.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Swytch: monitor input switch over DDC/CI (starts right at sign-in)</Description></RegistrationInfo>
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
"#,
        user = xml_esc(user),
        exe = xml_esc(&exe.to_string_lossy()),
        workdir = xml_esc(&workdir)
    )
}

/// Creates (or replaces) the logon task that starts `exe`.
pub fn install_task(exe: &Path) -> bool {
    let xml = task_xml(&user_name(), exe);
    let path = data_dir().join("task.xml");
    let mut bytes = vec![0xFFu8, 0xFE]; // UTF-16LE BOM
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    if let Err(e) = fs::create_dir_all(data_dir()).and_then(|_| fs::write(&path, &bytes)) {
        log(&format!("Start at sign-in: writing {path:?} failed: {e}"));
        return false;
    }
    let ok = run_hidden(
        "schtasks.exe",
        &["/Create".as_ref(), "/TN".as_ref(), TASK_NAME.as_ref(), "/XML".as_ref(), path.as_os_str(), "/F".as_ref()],
    );
    let _ = fs::remove_file(&path);
    if ok {
        // The task replaces the old Run value (which starts late).
        delete_run_value(APP_NAME);
    }
    log(&format!("Start at sign-in (scheduled task): install {}", if ok { "OK" } else { "FAILED" }));
    ok
}

pub fn uninstall_task() -> bool {
    let ok = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
    log(&format!("Start at sign-in: removal {}", if ok { "OK" } else { "FAILED" }));
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_escapes_and_sets_the_working_directory() {
        let xml = task_xml("PC\\a&b", Path::new(r"C:\Users\a&b\Programs\Swytch\Swytch.exe"));
        assert!(xml.contains("<UserId>PC\\a&amp;b</UserId>"));
        assert!(xml.contains(r#"<Command>"C:\Users\a&amp;b\Programs\Swytch\Swytch.exe"</Command>"#));
        assert!(xml.contains(r"<WorkingDirectory>C:\Users\a&amp;b\Programs\Swytch</WorkingDirectory>"));
        assert!(xml.contains("<LogonTrigger>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
    }
}
