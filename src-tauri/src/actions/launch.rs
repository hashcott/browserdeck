use super::ActionError;
use std::io;
use std::path::Path;

/// Opens a program, shortcut or document, with the file's own folder as working directory. `admin` asks Windows for
/// administrator rights (UAC) every time; that call returns only once someone answers the prompt on the PC, so it
/// runs on its own thread and the press counts as done at once (no press timeout while the owner walks to the PC).
/// A refused prompt is therefore not reported.
pub fn launch(path: &str, args: &[String], admin: bool) -> Result<(), ActionError> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(ActionError::FileNotFound);
    }
    let (route, verb) = route(crate::elevation::self_elevated(), admin);
    if admin {
        let (path, args) = (path.to_path_buf(), args.to_vec());
        std::thread::spawn(move || open(route, verb, &path, &args));
        return Ok(());
    }
    open(route, verb, path, args).map_err(|_| ActionError::LaunchFailed)
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Route {
    /// ShellExecute from this process.
    Direct,
    /// ShellExecute inside Explorer, so the program runs with Explorer's (standard) rights.
    Explorer,
}

/// An elevated app never hands its rights to what it opens: any program running as this user can change
/// config.json, and often the opened program's own files. So elevated, always through Explorer, as if double-clicked;
/// with no Explorer the launch fails rather than run directly. `admin` uses the "runas" verb, which shows UAC.
pub fn route(elevated: bool, admin: bool) -> (Route, &'static str) {
    (if elevated { Route::Explorer } else { Route::Direct }, if admin { "runas" } else { "open" })
}

/// The file's folder, or `None` for a bare file name.
fn work_dir(path: &Path) -> Option<&Path> {
    path.parent().filter(|dir| !dir.as_os_str().is_empty())
}

#[cfg(windows)]
fn open(route: Route, verb: &str, path: &Path, args: &[String]) -> io::Result<()> {
    let params = quote_args(args);
    match route {
        Route::Direct => {
            let verb = windows::core::HSTRING::from(verb);
            shell_execute(windows::core::PCWSTR(verb.as_ptr()), path, &params, work_dir(path))
        }
        Route::Explorer => shell_execute_via_explorer(verb, path, &params, work_dir(path)).map_err(io::Error::other),
    }
}

/// ShellExecute run by Explorer through its desktop window's shell automation object (IShellDispatch2), so the new
/// process gets Explorer's token, not this one's. Raymond Chen, "How can I launch an unelevated process from my
/// elevated process and vice versa?" (The Old New Thing, 2013-11-18). Explorer reports no result: a refused UAC
/// prompt still counts as opened.
#[cfg(windows)]
fn shell_execute_via_explorer(verb: &str, file: &Path, params: &str, dir: Option<&Path>) -> windows::core::Result<()> {
    use windows::core::{Interface, BSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, IDispatch, IServiceProvider, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        IShellBrowser, IShellDispatch2, IShellFolderViewDual, IShellView, IShellWindows, SID_STopLevelBrowser,
        ShellWindows, CSIDL_DESKTOP, SVGIO_BACKGROUND, SWC_DESKTOP, SWFO_NEEDDISPATCH,
    };
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let _com = super::ComGuard::init(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    let file = BSTR::from(file.to_string_lossy().as_ref());
    let dir = dir.map_or_else(VARIANT::default, |d| VARIANT::from(d.to_string_lossy().as_ref()));
    // SAFETY: COM calls on interfaces obtained just above, with arguments that outlive each call.
    unsafe {
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER)?;
        let mut hwnd = 0;
        let desktop = windows.FindWindowSW(
            &VARIANT::from(CSIDL_DESKTOP as i32),
            &VARIANT::default(),
            SWC_DESKTOP,
            &mut hwnd,
            SWFO_NEEDDISPATCH,
        )?;
        let browser: IShellBrowser = desktop.cast::<IServiceProvider>()?.QueryService(&SID_STopLevelBrowser)?;
        let view: IShellView = browser.QueryActiveShellView()?;
        // Ask for IDispatch, then query: the view hands out its background automation object for IID_IDispatch.
        let folder: IShellFolderViewDual = view.GetItemObject::<IDispatch>(SVGIO_BACKGROUND)?.cast()?;
        let shell: IShellDispatch2 = folder.Application()?.cast()?;
        shell.ShellExecute(&file, &VARIANT::from(params), &dir, &VARIANT::from(verb), &VARIANT::from(SW_SHOWNORMAL.0))
    }
}

/// Not Windows: never elevated, so always direct, and there is no "runas".
#[cfg(not(windows))]
fn open(_route: Route, _verb: &str, path: &Path, args: &[String]) -> io::Result<()> {
    let mut command = std::process::Command::new(path);
    command.args(args);
    if let Some(dir) = work_dir(path) {
        command.current_dir(dir);
    }
    let mut child = command.spawn()?;
    std::thread::spawn(move || child.wait()); // reap it so it never lingers as a zombie
    Ok(())
}

/// Runs `ShellExecuteW`; a result of 32 or less is the shell's error code.
#[cfg(windows)]
pub(crate) fn shell_execute(verb: windows::core::PCWSTR, file: &Path, params: &str, dir: Option<&Path>) -> io::Result<()> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    // Shortcuts and some file types run through COM shell extensions that need an STA.
    let _com = super::ComGuard::init(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    let (file, params, dir) = (HSTRING::from(file), HSTRING::from(params), dir.map(HSTRING::from));
    let dir = dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr()));
    // SAFETY: every string is NUL-terminated and outlives the call; a null directory means the current one.
    let code = unsafe { ShellExecuteW(None, verb, &file, &params, dir, SW_SHOWNORMAL) }.0 as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code as i32))
    }
}

/// Joins arguments into one Windows command line, quoting them so `CommandLineToArgvW` splits them back
/// exactly: backslashes are doubled only where they precede a quote or the closing quote.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn quote_args<S: AsRef<str>>(args: &[S]) -> String {
    let mut line = String::new();
    for arg in args {
        let arg = arg.as_ref();
        if !line.is_empty() {
            line.push(' ');
        }
        if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0B', '"']) {
            line.push_str(arg);
            continue;
        }
        line.push('"');
        let mut backslashes = 0;
        for c in arg.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    line.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                    line.push('"');
                    backslashes = 0;
                }
                _ => {
                    line.extend(std::iter::repeat_n('\\', backslashes));
                    line.push(c);
                    backslashes = 0;
                }
            }
        }
        line.extend(std::iter::repeat_n('\\', backslashes * 2));
        line.push('"');
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_file_not_found() {
        assert_eq!(launch("/definitely/not/here.exe", &[], false), Err(ActionError::FileNotFound));
    }

    #[test]
    fn elevated_always_goes_through_explorer_and_admin_always_asks() {
        assert_eq!(route(false, false), (Route::Direct, "open"));
        assert_eq!(route(false, true), (Route::Direct, "runas"));
        assert_eq!(route(true, false), (Route::Explorer, "open"));
        assert_eq!(route(true, true), (Route::Explorer, "runas"));
    }

    #[test]
    fn quotes_only_args_that_need_it() {
        assert_eq!(quote_args(&["--hidden", "plain"]), "--hidden plain");
        assert_eq!(quote_args(&["two words", ""]), r#""two words" """#);
        assert_eq!(quote_args(&[r#"say "hi""#]), r#""say \"hi\"""#);
        assert_eq!(quote_args(&[r"C:\a b\"]), r#""C:\a b\\""#);
        assert_eq!(quote_args(&[r#"a\"b c"#]), r#""a\\\"b c""#);
        assert_eq!(quote_args(&[r"C:\no\spaces"]), r"C:\no\spaces");
    }
}
