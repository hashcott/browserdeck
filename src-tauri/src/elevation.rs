//! Whether this process or the foreground window runs as administrator, and relaunching through UAC.
//! Windows drops simulated keys aimed at an elevated window from a non-elevated sender (UIPI).

use std::io;

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
