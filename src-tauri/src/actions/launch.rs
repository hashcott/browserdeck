use super::ActionError;
use std::io;
use std::path::Path;

/// Opens a program, shortcut or document, with the file's own folder as working directory.
pub fn launch(path: &str, args: &[String]) -> Result<(), ActionError> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(ActionError::FileNotFound);
    }
    open(path, args).map_err(|_| ActionError::LaunchFailed)
}

/// The file's folder, or `None` for a bare file name.
fn work_dir(path: &Path) -> Option<&Path> {
    path.parent().filter(|dir| !dir.as_os_str().is_empty())
}

#[cfg(windows)]
fn open(path: &Path, args: &[String]) -> io::Result<()> {
    shell_execute(windows::core::w!("open"), path, &quote_args(args), work_dir(path))
}

#[cfg(not(windows))]
fn open(path: &Path, args: &[String]) -> io::Result<()> {
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
        assert_eq!(launch("/definitely/not/here.exe", &[]), Err(ActionError::FileNotFound));
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
