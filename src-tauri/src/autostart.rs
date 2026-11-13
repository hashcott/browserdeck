//! Start with Windows. Without administrator mode: the per-user Run key (tauri-plugin-autostart, `--hidden`).
//! With it: a scheduled task at this user's logon with highest privileges, so signing in never shows a UAC prompt.

use crate::commands::SystemSettingError;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

pub const TASK_NAME: &str = "browserdeck-autostart";
/// Start without showing the window.
pub const HIDDEN_ARG: &str = "--hidden";

#[derive(Debug, PartialEq)]
pub enum Mechanism {
    None,
    RunKey,
    ElevatedTask,
}

pub fn mechanism(autostart: bool, run_as_admin: bool) -> Mechanism {
    match (autostart, run_as_admin) {
        (false, _) => Mechanism::None,
        (true, false) => Mechanism::RunKey,
        (true, true) => Mechanism::ElevatedTask,
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// The scheduled task. `schtasks /Create /TR` would take Task Scheduler's defaults: start only on AC power, stop
/// on battery, stop after 3 days, below-normal priority, trigger at any user's logon.
pub fn task_xml(exe: &str, user: &str) -> String {
    let (exe, user) = (xml_escape(exe), xml_escape(user));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{HIDDEN_ARG}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// UTF-16LE with a byte order mark, the encoding `schtasks /XML` reads.
#[cfg_attr(not(windows), allow(dead_code))]
fn utf16_bom(s: &str) -> Vec<u8> {
    [0xFF, 0xFE].into_iter().chain(s.encode_utf16().flat_map(u16::to_le_bytes)).collect()
}

/// Installs the mechanism the settings pick and removes the other one (one that is not there is fine).
/// Blocks while schtasks runs.
pub fn apply(app: &AppHandle, autostart: bool, run_as_admin: bool) -> Result<(), SystemSettingError> {
    let run_key = app.autolaunch();
    let plugin = |_| SystemSettingError::AutostartFailed;
    match mechanism(autostart, run_as_admin) {
        Mechanism::None => {
            remove_task()?;
            run_key.disable().map_err(plugin)
        }
        Mechanism::RunKey => {
            // Run elevated, the plugin writes the Run key for every user of the PC (HKLM). Leave the per-user entry
            // to the next normal start, which applies the settings again (see lib.rs).
            if !crate::elevation::self_elevated() {
                run_key.enable().map_err(plugin)?;
            }
            remove_task()
        }
        Mechanism::ElevatedTask => {
            // The Run key goes first, whatever happens to the task: left behind, it would start the app unelevated
            // at every sign-in, and administrator mode would then ask for UAC each time.
            run_key.disable().map_err(plugin)?;
            create_task()
        }
    }
}

#[cfg(windows)]
fn create_task() -> Result<(), SystemSettingError> {
    use std::ffi::OsStr;

    // A task with highest privileges must never run an exe that a standard user can replace.
    if !crate::elevation::machine_install() {
        return Err(SystemSettingError::NeedsMachineInstall);
    }
    let exe = std::env::current_exe().map_err(|_| SystemSettingError::AutostartFailed)?;
    let exe_str = exe.to_str().ok_or(SystemSettingError::AutostartFailed)?;
    let xml = task_xml(exe_str, &current_user().ok_or(SystemSettingError::AutostartFailed)?);
    // Next to the exe, not in %TEMP%: the install folder is admin-only (machine install), so nothing can swap the file.
    let file = exe.with_file_name(format!("{TASK_NAME}.xml"));
    let created = std::fs::write(&file, utf16_bom(&xml))
        .and_then(|()| schtasks(&[OsStr::new("/Create"), OsStr::new("/TN"), OsStr::new(TASK_NAME), OsStr::new("/XML"), file.as_os_str(), OsStr::new("/F")]));
    let _ = std::fs::remove_file(&file);
    match created {
        Ok(true) => Ok(()),
        _ => Err(SystemSettingError::AutostartFailed),
    }
}

#[cfg(windows)]
fn remove_task() -> Result<(), SystemSettingError> {
    // schtasks exits with 1 for every failure and words it in the system language, so "no such task" comes from
    // the task store instead: Task Scheduler keeps task \<name> in System32\Tasks\<name>. Only a lookup that finds
    // nothing counts as "no task"; otherwise /Delete must succeed.
    let missing = system_dir()
        .map(|d| d.join("Tasks").join(TASK_NAME))
        .is_ok_and(|f| matches!(std::fs::metadata(f), Err(e) if e.kind() == std::io::ErrorKind::NotFound));
    if missing || schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]).unwrap_or(false) {
        Ok(())
    } else {
        Err(SystemSettingError::AutostartFailed)
    }
}

/// `DOMAIN\user` of this process's user, the form Task Scheduler writes in `<UserId>`.
#[cfg(windows)]
fn current_user() -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Security::Authentication::Identity::{GetUserNameExW, NameSamCompatible};

    let mut buf = [0u16; 512];
    let mut len = buf.len() as u32;
    // SAFETY: the buffer is live and `len` holds its length in characters; on success `len` excludes the NUL.
    unsafe { GetUserNameExW(NameSamCompatible, Some(PWSTR(buf.as_mut_ptr())), &mut len) }
        .then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

#[cfg(windows)]
fn system_dir() -> std::io::Result<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

    let mut buf = [0u16; 260];
    // SAFETY: the buffer is live and its length is passed with it.
    let len = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    if len == 0 || len > buf.len() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(std::ffi::OsString::from_wide(&buf[..len]).into())
}

/// Runs System32\schtasks.exe directly: no shell, no PATH search, no console window. Ok(false) = it reported failure.
#[cfg(windows)]
fn schtasks<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> std::io::Result<bool> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;

    let status = Command::new(system_dir()?.join("schtasks.exe"))
        .args(args)
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

#[cfg(not(windows))]
fn create_task() -> Result<(), SystemSettingError> {
    Err(SystemSettingError::UnsupportedPlatform)
}

/// No scheduled tasks here, so there is never one to remove.
#[cfg(not(windows))]
fn remove_task() -> Result<(), SystemSettingError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mechanism_matrix() {
        assert!(matches!(mechanism(false, false), Mechanism::None));
        assert!(matches!(mechanism(false, true), Mechanism::None));
        assert!(matches!(mechanism(true, false), Mechanism::RunKey));
        assert!(matches!(mechanism(true, true), Mechanism::ElevatedTask));
    }

    fn between<'a>(s: &'a str, open: &str, close: &str) -> &'a str {
        let start = s.find(open).unwrap_or_else(|| panic!("no {open}")) + open.len();
        &s[start..start + s[start..].find(close).unwrap_or_else(|| panic!("no {close}"))]
    }

    #[test]
    fn task_runs_at_this_users_logon_elevated_with_settings_fit_for_a_long_running_app() {
        let x = task_xml(r"C:\Program Files\Browserdeck\browserdeck.exe", r"PC\me");
        assert_eq!(TASK_NAME, "browserdeck-autostart");
        assert!(x.starts_with(r#"<?xml version="1.0" encoding="UTF-16"?>"#));
        assert_eq!(between(&x, "<Command>", "</Command>"), r"C:\Program Files\Browserdeck\browserdeck.exe");
        assert_eq!(between(&x, "<Arguments>", "</Arguments>"), "--hidden");
        assert_eq!(between(between(&x, "<LogonTrigger>", "</LogonTrigger>"), "<UserId>", "</UserId>"), r"PC\me");
        let principal = between(&x, "<Principal ", "</Principal>");
        assert_eq!(between(principal, "<UserId>", "</UserId>"), r"PC\me");
        assert_eq!(between(principal, "<LogonType>", "</LogonType>"), "InteractiveToken");
        assert_eq!(between(principal, "<RunLevel>", "</RunLevel>"), "HighestAvailable");
        let settings = between(&x, "<Settings>", "</Settings>");
        for (tag, value) in [
            ("MultipleInstancesPolicy", "IgnoreNew"),
            ("DisallowStartIfOnBatteries", "false"),
            ("StopIfGoingOnBatteries", "false"),
            ("ExecutionTimeLimit", "PT0S"),
            ("Priority", "5"),
        ] {
            assert_eq!(between(settings, &format!("<{tag}>"), &format!("</{tag}>")), value, "{tag}");
        }
    }

    #[test]
    fn task_xml_escapes_path_and_user() {
        let x = task_xml(r#"C:\A&B <x> "q" 'a'\app.exe"#, "PC\\<me&you>");
        assert_eq!(between(&x, "<Command>", "</Command>"), r"C:\A&amp;B &lt;x&gt; &quot;q&quot; &apos;a&apos;\app.exe");
        assert_eq!(between(&x, "<UserId>", "</UserId>"), r"PC\&lt;me&amp;you&gt;");
    }

    #[test]
    fn task_file_is_utf16le_with_bom() {
        assert_eq!(utf16_bom("<a>é"), [0xFF, 0xFE, b'<', 0, b'a', 0, b'>', 0, 0xE9, 0]);
    }
}
