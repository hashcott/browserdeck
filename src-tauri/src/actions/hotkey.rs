use super::keymap::scancode;
use super::ActionError;

/// One key transition: (scan code without the `0xE0` prefix, extended key, key-up).
type KeyEvent = (u16, bool, bool);

/// Presses the chord's keys in order, then releases them in reverse order.
/// Order of checks: invalid chord, then platform, then elevation, then sending.
pub fn send_chord(codes: &[String]) -> Result<(), ActionError> {
    let events = chord_events(codes)?;
    send(&events)
}

/// Turns a chord into its key events: every key down in order, then every key up in reverse.
fn chord_events(codes: &[String]) -> Result<Vec<KeyEvent>, ActionError> {
    let scans: Vec<u16> = codes.iter().map(|c| scancode(c)).collect::<Option<_>>().ok_or(ActionError::InvalidHotkey)?;
    if scans.is_empty() {
        return Err(ActionError::InvalidHotkey);
    }
    let event = |scan: u16, up: bool| (scan & 0xFF, scan >> 8 == 0xE0, up);
    let downs = scans.iter().map(|&s| event(s, false));
    let ups = scans.iter().rev().map(|&s| event(s, true));
    Ok(downs.chain(ups).collect())
}

#[cfg(not(windows))]
fn send(_: &[KeyEvent]) -> Result<(), ActionError> {
    Err(ActionError::UnsupportedPlatform)
}

#[cfg(windows)]
fn send(events: &[KeyEvent]) -> Result<(), ActionError> {
    use crate::elevation::{foreground_elevated, self_elevated};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, VIRTUAL_KEY,
    };

    // Windows silently drops input aimed at an elevated window from a non-elevated sender (UIPI).
    if !self_elevated() && foreground_elevated() {
        return Err(ActionError::BlockedByElevatedWindow);
    }
    let inputs: Vec<INPUT> = events
        .iter()
        .map(|&(scan, extended, up)| {
            let mut flags = KEYEVENTF_SCANCODE;
            if extended {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            let ki = KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 };
            INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki } }
        })
        .collect();
    // SAFETY: `inputs` is a live slice of fully initialised INPUT values and cbSize is their exact size.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    // SendInput fails in practice when UIPI or the secure desktop (UAC prompt, lock screen) blocks it.
    // ponytail: no key-up recovery after a partial send; add it if stuck modifiers are ever reported.
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(ActionError::BlockedByElevatedWindow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chord_is_invalid() {
        assert_eq!(send_chord(&[]), Err(ActionError::InvalidHotkey));
    }

    #[test]
    fn unmapped_code_is_invalid_and_sends_nothing() {
        assert_eq!(send_chord(&["KeyM".into(), "Nope".into()]), Err(ActionError::InvalidHotkey));
    }

    #[test]
    fn presses_in_order_and_releases_in_reverse() {
        let chord = ["ControlLeft".to_string(), "ShiftLeft".into(), "ArrowUp".into()];
        assert_eq!(
            chord_events(&chord),
            Ok(vec![
                (0x1D, false, false),
                (0x2A, false, false),
                (0x48, true, false),
                (0x48, true, true),
                (0x2A, false, true),
                (0x1D, false, true),
            ])
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn valid_chord_is_unsupported_off_windows() {
        assert_eq!(send_chord(&["ControlLeft".into(), "KeyM".into()]), Err(ActionError::UnsupportedPlatform));
    }
}
