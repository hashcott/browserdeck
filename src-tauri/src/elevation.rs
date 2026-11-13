//! Whether this process or the foreground window runs as administrator, and relaunching through UAC.
//! Windows drops simulated keys aimed at an elevated window from a non-elevated sender (UIPI).

use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};

/// Whether this process runs elevated. Unknown counts as not elevated, so the foreground check still runs.
#[cfg(windows)]
pub fn self_elevated() -> bool {
    // SAFETY: returns this process's pseudo-handle, which is always valid and is never closed.
    let process = unsafe { windows::Win32::System::Threading::GetCurrentProcess() };
    token_elevated(process).unwrap_or(false)
}

/// Whether the foreground window belongs to an elevated process. Fails safe: a process whose
/// elevation cannot be read (for example access denied) counts as elevated.
#[cfg(windows)]
pub fn foreground_elevated() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // SAFETY: takes no arguments; a null window is handled below.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return false; // nothing has focus, so no window can be the one blocking input
    }
    let mut pid = 0;
    // SAFETY: `pid` is a live u32 for the call to write; a window gone stale makes it return 0.
    if unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) } == 0 {
        return true;
    }
    process_elevated(pid).unwrap_or(true)
}

#[cfg(windows)]
fn process_elevated(pid: u32) -> windows::core::Result<bool> {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: plain value arguments; the returned handle is closed below on every path.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?;
    let elevated = token_elevated(process);
    // SAFETY: `process` was opened above and is closed exactly once.
    unsafe { CloseHandle(process) }?;
    elevated
}

#[cfg(windows)]
fn token_elevated(process: HANDLE) -> windows::core::Result<bool> {
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::OpenProcessToken;

    let mut token = HANDLE::default();
    // SAFETY: `process` is a valid process handle and `token` a live out-parameter.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut written = 0;
    let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
    let buffer = Some(&mut elevation as *mut TOKEN_ELEVATION as *mut std::ffi::c_void);
    // SAFETY: `buffer` points to a live TOKEN_ELEVATION and `size` is its exact size.
    let queried = unsafe { GetTokenInformation(token, TokenElevation, buffer, size, &mut written) };
    // SAFETY: `token` was opened above and is closed exactly once.
    let closed = unsafe { CloseHandle(token) };
    queried.and(closed)?;
    Ok(elevation.TokenIsElevated != 0)
}

/// Starts this executable again through the UAC prompt; a refused prompt is an error.
#[cfg(windows)]
pub fn relaunch_elevated(extra_args: &[&str]) -> io::Result<()> {
    use crate::actions::launch::{quote_args, shell_execute};

    let exe = std::env::current_exe()?;
    shell_execute(windows::core::w!("runas"), &exe, &quote_args(extra_args), None)
}

/// Marks the copy started by the startup relaunch. That copy never relaunches again, so elevation can never loop.
pub const ELEVATED_CHILD_ARG: &str = "--elevated-child";
/// `--wait-pid=<pid>`: the process this copy replaces, which still holds the phone port and the single-instance lock.
pub const WAIT_PID_ARG: &str = "--wait-pid=";

/// Startup rule: administrator mode is on, this process is not elevated, and it is not the relaunched copy.
pub fn should_relaunch(run_as_admin: bool, self_elevated: bool, args: &[String]) -> bool {
    run_as_admin && !self_elevated && !args.iter().any(|a| a == ELEVATED_CHILD_ARG)
}

/// Lower case, `\` separators, no `\\?\` prefix, `.` and `..` resolved, no empty or trailing segment.
fn normalise(path: &Path) -> String {
    let path = path.to_string_lossy().replace('/', "\\");
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path);
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop(); // never above the drive
                }
            }
            _ => parts.push(part),
        }
    }
    parts.join("\\").to_lowercase()
}

/// Whether `exe` lies inside one of the Program Files folders, which only administrators can write to.
pub fn is_machine_install(exe: &Path, program_files_dirs: &[PathBuf]) -> bool {
    let exe = normalise(exe);
    program_files_dirs.iter().map(|d| normalise(d)).any(|dir| {
        !dir.is_empty() && exe.strip_prefix(&dir).is_some_and(|rest| rest.starts_with('\\'))
    })
}

/// Administrator mode (the UAC relaunch and the elevated logon task) needs this: an elevated app must not run an
/// exe, or load DLLs from a folder, that a standard user can change.
#[cfg(windows)]
pub fn machine_install() -> bool {
    std::env::current_exe().is_ok_and(|exe| is_machine_install(&exe, &program_files_dirs()))
}

#[cfg(not(windows))]
pub fn machine_install() -> bool {
    false
}

/// From the known folders, not the environment, which an unelevated process can set for its children.
#[cfg(windows)]
fn program_files_dirs() -> Vec<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        SHGetKnownFolderPath, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX64, FOLDERID_ProgramFilesX86, KF_FLAG_DEFAULT,
    };

    [FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86, FOLDERID_ProgramFilesX64]
        .iter()
        .filter_map(|id| {
            // SAFETY: `id` is a known-folder GUID; the returned string is freed below.
            let path = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok()?;
            // SAFETY: `path` is the NUL-terminated string SHGetKnownFolderPath returned.
            let dir = unsafe { path.to_string() }.ok().map(PathBuf::from);
            // SAFETY: allocated by SHGetKnownFolderPath with CoTaskMemAlloc, freed exactly once.
            unsafe { CoTaskMemFree(Some(path.0 as *const _)) };
            dir
        })
        .collect()
}

/// The names WebView2 reads to pick its runtime folder (where it loads DLLs from), data folder and browser
/// arguments. Windows variable names ignore case.
pub fn webview2_vars<I: IntoIterator<Item = String>>(names: I) -> Vec<String> {
    names.into_iter().filter(|n| n.get(..9).is_some_and(|p| p.eq_ignore_ascii_case("WEBVIEW2_"))).collect()
}

/// An elevated copy inherits the user's environment (HKCU\Environment), which any program running as this user
/// can write. So when elevated: drop every WEBVIEW2_* variable, otherwise one could point WebView2 at a folder of
/// its own DLLs, and stop searching PATH and the current folder for DLLs. Must run before any thread starts.
pub fn harden_if_elevated() {
    if !self_elevated() {
        return;
    }
    let names = std::env::vars_os().filter_map(|(name, _)| name.into_string().ok());
    for name in webview2_vars(names) {
        std::env::remove_var(name);
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS};
        use windows::Win32::System::SystemServices::{
            PROCESS_MITIGATION_REDIRECTION_TRUST_POLICY, PROCESS_MITIGATION_REDIRECTION_TRUST_POLICY_0,
        };
        use windows::Win32::System::Threading::{ProcessRedirectionTrustPolicy, SetProcessMitigationPolicy};

        // SAFETY: plain flag argument. It only fails before Windows 8, which Tauri 2 does not run on.
        let _ = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) };
        // Redirection Guard (EnforceRedirectionTrust, bit 0): never follow a junction a standard user made, so a
        // junction in the user's app data folder cannot turn this app's file writes into writes anywhere.
        // Windows versions without it refuse the call; nothing else changes then.
        let policy = PROCESS_MITIGATION_REDIRECTION_TRUST_POLICY {
            Anonymous: PROCESS_MITIGATION_REDIRECTION_TRUST_POLICY_0 { Flags: 1 },
        };
        // SAFETY: `policy` is a live struct of the size passed, the layout this policy takes.
        let _ = unsafe {
            SetProcessMitigationPolicy(
                ProcessRedirectionTrustPolicy,
                &policy as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&policy),
            )
        };
        // The runtime installed for all users, so a runtime registered for this user only (user-writable) is never
        // loaded. Without one, WebView2 picks as usual.
        if let Some(dir) = machine_webview2() {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", dir);
        }
        // Redirection Guard does not pass to child processes, and the elevated WebView2 browser process writes and
        // deletes many files: keep its data and its temporary files (TMP/TEMP) out of the user's folders. Only with
        // a machine install, which administrator mode requires; a per-user copy someone elevated by hand is out of
        // scope (its exe can be replaced anyway).
        if machine_install() {
            if let Some(data) = std::env::current_exe().ok().and_then(|exe| admin_webview2_data(&exe)) {
                let temp = data.join("Temp");
                if std::fs::create_dir_all(&temp).is_ok() {
                    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &data);
                    std::env::set_var("TMP", &temp);
                    std::env::set_var("TEMP", &temp);
                }
            }
        }
    }
}

/// The elevated copy's WebView2 data folder, next to the exe: with a machine install only administrators can write
/// there, unlike the default under the user's AppData, where a junction could redirect what the elevated browser
/// process writes and deletes. None for a bare file name.
pub fn admin_webview2_data(exe: &Path) -> Option<PathBuf> {
    exe.parent().filter(|dir| !dir.as_os_str().is_empty()).map(|dir| dir.join("WebView2Admin"))
}

/// `location\pv` when both are set and the folder holds msedgewebview2.exe.
pub fn webview2_runtime_folder(
    location: Option<String>,
    pv: Option<String>,
    has_exe: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let (location, pv) = (location.filter(|s| !s.is_empty())?, pv.filter(|s| !s.is_empty())?);
    let dir = Path::new(&location).join(pv);
    has_exe(&dir.join("msedgewebview2.exe")).then_some(dir)
}

/// The Evergreen WebView2 runtime installed for all users, from its EdgeUpdate entry in HKLM (32-bit view), which
/// only administrators can change.
#[cfg(windows)]
fn machine_webview2() -> Option<PathBuf> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6432KEY};

    let key = HSTRING::from(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}");
    let read = |name: &str| {
        let mut buf = [0u16; 1024];
        let mut bytes = std::mem::size_of_val(&buf) as u32;
        // SAFETY: `buf` is live and `bytes` holds its size in bytes; RRF_RT_REG_SZ NUL-terminates the result.
        unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                &key,
                &HSTRING::from(name),
                RRF_RT_REG_SZ | RRF_SUBKEY_WOW6432KEY,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        }
        .ok()
        .ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16(&buf[..len]).ok()
    };
    webview2_runtime_folder(read("location"), read("pv"), Path::is_file)
}

pub fn wait_pid(args: &[String]) -> Option<u32> {
    args.iter().find_map(|a| a.strip_prefix(WAIT_PID_ARG)?.parse().ok())
}

/// Waits up to 10 s for process `pid` to end; returns at once if it already has.
#[cfg(windows)]
pub fn wait_for_exit(pid: u32) {
    use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

    // SAFETY: plain value arguments; the returned handle is closed below.
    let Ok(process) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else { return };
    // SAFETY: `process` is the handle opened above, still open.
    unsafe { WaitForSingleObject(process, 10_000) };
    // SAFETY: `process` was opened above and is closed exactly once.
    let _ = unsafe { CloseHandle(process) };
}

#[cfg(not(windows))]
pub fn wait_for_exit(_pid: u32) {}

#[cfg(not(windows))]
pub fn self_elevated() -> bool {
    false
}

#[cfg(not(windows))]
pub fn foreground_elevated() -> bool {
    false
}

#[cfg(not(windows))]
pub fn relaunch_elevated(_extra_args: &[&str]) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn relaunches_only_when_admin_mode_wants_it_and_never_twice() {
        let plain = args(&["app.exe"]);
        let child = args(&["app.exe", "--elevated-child"]);
        assert!(should_relaunch(true, false, &plain));
        assert!(!should_relaunch(true, true, &plain));
        assert!(!should_relaunch(false, false, &plain));
        assert!(!should_relaunch(false, true, &plain));
        // the relaunched copy keeps running unelevated rather than asking again
        assert!(!should_relaunch(true, false, &child));
        assert!(!should_relaunch(true, true, &child));
        assert!(!should_relaunch(true, false, &args(&["app.exe", "--hidden", "--elevated-child"])));
        assert!(should_relaunch(true, false, &args(&["app.exe", "--hidden"])));
    }

    #[test]
    fn machine_install_means_under_program_files() {
        let dirs = [PathBuf::from(r"C:\Program Files"), PathBuf::from(r"C:\Program Files (x86)\")];
        let yes = |p: &str| assert!(is_machine_install(Path::new(p), &dirs), "{p}");
        let no = |p: &str| assert!(!is_machine_install(Path::new(p), &dirs), "{p}");
        yes(r"C:\Program Files\Browserdeck\browserdeck.exe");
        yes(r"c:\PROGRAM FILES\Browserdeck\browserdeck.exe");
        yes(r"C:\Program Files (x86)\Browserdeck\browserdeck.exe");
        yes(r"\\?\C:\Program Files\Browserdeck\browserdeck.exe");
        yes(r"C:/Program Files/Browserdeck/./browserdeck.exe");
        no(r"C:\Program Files Evil\browserdeck.exe");
        no(r"C:\Program Filesx\browserdeck.exe");
        no(r"C:\Users\me\AppData\Local\Browserdeck\browserdeck.exe");
        no(r"C:\Program Files\..\Users\me\browserdeck.exe");
        no(r"C:\Program Files\Browserdeck\..\..\Users\browserdeck.exe");
        no(r"C:\Program Files");
        no(r"D:\Program Files\Browserdeck\browserdeck.exe");
        assert!(!is_machine_install(Path::new(r"C:\Program Files\B\b.exe"), &[]));
        assert!(!is_machine_install(Path::new(r"C:\b.exe"), &[PathBuf::new()]));
    }

    #[test]
    fn picks_every_webview2_variable_whatever_its_case() {
        let names = [
            "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
            "webview2_additional_browser_arguments",
            "WebView2_User_Data_Folder",
            "PATH",
            "MY_WEBVIEW2_X",
            "WEBVIEW2",
        ]
        .map(String::from);
        assert_eq!(
            webview2_vars(names),
            ["WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", "webview2_additional_browser_arguments", "WebView2_User_Data_Folder"]
        );
    }

    #[test]
    fn webview2_folder_is_location_and_version_when_the_runtime_is_there() {
        let loc = || Some(r"C:\P\EdgeWebView\Application".to_string());
        let pv = || Some("131.0.2903.70".to_string());
        let want = Path::new(r"C:\P\EdgeWebView\Application").join("131.0.2903.70");
        assert_eq!(
            webview2_runtime_folder(loc(), pv(), |exe| exe == want.join("msedgewebview2.exe")),
            Some(want.clone())
        );
        assert_eq!(webview2_runtime_folder(loc(), None, |_| true), None);
        assert_eq!(webview2_runtime_folder(loc(), Some(String::new()), |_| true), None);
        assert_eq!(webview2_runtime_folder(None, pv(), |_| true), None);
        assert_eq!(webview2_runtime_folder(Some(String::new()), pv(), |_| true), None);
        assert_eq!(webview2_runtime_folder(loc(), pv(), |_| false), None);
    }

    #[test]
    fn admin_webview2_data_sits_next_to_the_exe() {
        // `/` separates on every platform the tests run on.
        let exe = Path::new("C:/Program Files/Browserdeck/browserdeck.exe");
        assert_eq!(admin_webview2_data(exe), Some(Path::new("C:/Program Files/Browserdeck").join("WebView2Admin")));
        assert_eq!(admin_webview2_data(Path::new("browserdeck.exe")), None);
    }

    #[test]
    fn wait_pid_reads_the_predecessor() {
        assert_eq!(wait_pid(&args(&["app.exe", "--wait-pid=4242"])), Some(4242));
        assert_eq!(wait_pid(&args(&["app.exe", "--hidden"])), None);
        assert_eq!(wait_pid(&args(&["app.exe", "--wait-pid=x"])), None);
    }
}
