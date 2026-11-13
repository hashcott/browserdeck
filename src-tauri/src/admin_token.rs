//! The pairing token of an elevated copy. config.json is readable and writable by every program running as this
//! user, so a token kept there would let any of them press buttons in the elevated app (shortcut keys reach
//! administrator windows). The elevated copy keeps its own token in HKLM instead, in a key only SYSTEM and
//! Administrators can open. Standard users cannot create keys under HKLM\SOFTWARE, so an existing key was made by an
//! administrator.

use std::io;

/// Under HKEY_LOCAL_MACHINE. One value per Windows user, named by the user's SID.
pub const KEY: &str = r"SOFTWARE\Browserdeck\AdminTokens";
/// Protected DACL (nothing inherited from SOFTWARE): full access for SYSTEM and Administrators, nobody else.
pub const KEY_SDDL: &str = "D:P(A;CI;KA;;;SY)(A;CI;KA;;;BA)";

/// The token this process's phone server accepts. None: not elevated, use config.json's. Elevated, a token that
/// cannot be loaded is an error code for `Startup::server_error`, never a fall back to config.json's.
pub fn for_process(elevated: bool, load: impl FnOnce() -> io::Result<String>) -> Result<Option<String>, &'static str> {
    if !elevated {
        return Ok(None);
    }
    match load() {
        Ok(token) if !token.is_empty() => Ok(Some(token)),
        _ => Err("admin_token_failed"),
    }
}

/// This user's stored token, or a new one stored now.
#[cfg(windows)]
pub fn load_or_create() -> io::Result<String> {
    let (key, name) = (Key::open()?, user_sid()?);
    match key.read(&name) {
        Some(token) if !token.is_empty() => Ok(token),
        _ => {
            let token = crate::auth::new_token();
            key.write(&name, &token)?;
            Ok(token)
        }
    }
}

#[cfg(windows)]
pub fn store(token: &str) -> io::Result<()> {
    Key::open()?.write(&user_sid()?, token)
}

#[cfg(not(windows))]
pub fn load_or_create() -> io::Result<String> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(not(windows))]
pub fn store(_token: &str) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(windows)]
struct Key(windows::Win32::System::Registry::HKEY);

#[cfg(windows)]
impl Key {
    /// Creates the key with `KEY_SDDL` if it is missing; an existing key keeps the descriptor it was made with.
    fn open() -> io::Result<Key> {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        use windows::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
        };
        use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
        use windows::Win32::System::Registry::{
            RegCreateKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE,
        };

        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the SDDL string is NUL-terminated and outlives the call; `descriptor` is a live out-parameter,
        // freed below with LocalFree.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &HSTRING::from(KEY_SDDL),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(io::Error::other)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let mut key = HKEY::default();
        // SAFETY: every pointer refers to a live local for the duration of the call.
        let created = unsafe {
            RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                &HSTRING::from(KEY),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE,
                Some(&attributes),
                &mut key,
                None,
            )
        };
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW, freed exactly once.
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        created.ok().map_err(io::Error::other)?;
        Ok(Key(key))
    }

    /// None when the value is missing, not a string, or cannot be read.
    fn read(&self, name: &str) -> Option<String> {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};

        let mut buf = [0u16; 256];
        let mut bytes = std::mem::size_of_val(&buf) as u32;
        // SAFETY: `buf` is live and `bytes` holds its size in bytes; RRF_RT_REG_SZ NUL-terminates the result.
        unsafe {
            RegGetValueW(
                self.0,
                None,
                &HSTRING::from(name),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        }
        .ok()
        .ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16(&buf[..len]).ok()
    }

    fn write(&self, name: &str, value: &str) -> io::Result<()> {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{RegSetValueExW, REG_SZ};

        let data: Vec<u8> = value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        // SAFETY: the key is open; the name is NUL-terminated; `data` is the UTF-16LE string with its NUL.
        unsafe { RegSetValueExW(self.0, &HSTRING::from(name), None, REG_SZ, Some(&data)) }
            .ok()
            .map_err(io::Error::other)
    }
}

#[cfg(windows)]
impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: opened by RegCreateKeyExW and closed exactly once.
        let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(self.0) };
    }
}

/// The SID of this process's user, as a string (S-1-5-21-...).
#[cfg(windows)]
fn user_sid() -> io::Result<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = HANDLE::default();
    // SAFETY: the current-process pseudo-handle is always valid; `token` is a live out-parameter, closed below.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.map_err(io::Error::other)?;
    // TOKEN_USER plus the SID it points to; u64 elements keep the buffer aligned for TOKEN_USER.
    let mut buf = [0u64; 16];
    let mut written = 0;
    // SAFETY: `buf` is live and its size in bytes is passed with it.
    let queried = unsafe {
        GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), std::mem::size_of_val(&buf) as u32, &mut written)
    };
    // SAFETY: opened above, closed exactly once.
    let _ = unsafe { CloseHandle(token) };
    queried.map_err(io::Error::other)?;
    // SAFETY: GetTokenInformation succeeded, so `buf` starts with a TOKEN_USER whose SID lies inside `buf`.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = PWSTR::null();
    // SAFETY: `sid` is valid while `buf` lives; `text` is freed below with LocalFree.
    unsafe { ConvertSidToStringSidW(sid, &mut text) }.map_err(io::Error::other)?;
    // SAFETY: ConvertSidToStringSidW returned a NUL-terminated string.
    let sid = unsafe { text.to_string() }.map_err(io::Error::other);
    // SAFETY: allocated by ConvertSidToStringSidW, freed exactly once.
    unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    sid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unelevated_uses_the_file_token_and_elevated_never_falls_back_to_it() {
        assert_eq!(for_process(false, || panic!("must not touch the registry")), Ok(None));
        assert_eq!(for_process(true, || Ok("t".into())), Ok(Some("t".into())));
        assert_eq!(for_process(true, || Err(std::io::ErrorKind::PermissionDenied.into())), Err("admin_token_failed"));
        assert_eq!(for_process(true, || Ok(String::new())), Err("admin_token_failed"));
    }
}
