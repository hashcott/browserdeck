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
    fn wait_pid_reads_the_predecessor() {
        assert_eq!(wait_pid(&args(&["app.exe", "--wait-pid=4242"])), Some(4242));
        assert_eq!(wait_pid(&args(&["app.exe", "--hidden"])), None);
        assert_eq!(wait_pid(&args(&["app.exe", "--wait-pid=x"])), None);
    }
}
