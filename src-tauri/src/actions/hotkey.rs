use super::keymap::scancode;
use super::ActionError;
use std::time::Duration;

/// How long every key of a chord stays down. Apps that poll key state (games, OBS's hotkey thread)
/// miss a press shorter than their polling interval, so raise this if a target app misses presses.
pub const KEY_HOLD: Duration = Duration::from_millis(30);

/// One key transition: (scan code without the `0xE0` prefix, extended key, key-up).
type KeyEvent = (u16, bool, bool);

/// Presses the chord's keys in order, holds them for `KEY_HOLD`, then releases them in reverse order.
/// Order of checks: invalid chord, then platform, then elevation, then sending. Returns
/// `BlockedByElevatedWindow` when the foreground window is elevated and we are not, or when Windows
/// accepts fewer key events than sent, which also covers the lock screen and the UAC secure desktop.
pub fn send_chord(codes: &[String]) -> Result<(), ActionError> {
    let (downs, ups) = chord_events(codes)?;
    send(&downs, &ups)
}

/// Turns a chord into its key-downs, in order, and its key-ups, in reverse order.
fn chord_events(codes: &[String]) -> Result<(Vec<KeyEvent>, Vec<KeyEvent>), ActionError> {
    let scans: Vec<u16> = codes.iter().map(|c| scancode(c)).collect::<Option<_>>().ok_or(ActionError::InvalidHotkey)?;
    if scans.is_empty() {
        return Err(ActionError::InvalidHotkey);
    }
    let event = |scan: u16, up: bool| (scan & 0xFF, scan >> 8 == 0xE0, up);
    let downs = scans.iter().map(|&s| event(s, false)).collect();
    let ups = scans.iter().rev().map(|&s| event(s, true)).collect();
    Ok((downs, ups))
}

/// The key-ups that release exactly the first `pressed` key-downs, last pressed first.
/// `ups` is the downs in reverse, so these are its last `pressed` entries.
#[cfg_attr(not(windows), allow(dead_code))]
fn releases(ups: &[KeyEvent], pressed: usize) -> &[KeyEvent] {
    &ups[ups.len().saturating_sub(pressed)..]
}

#[cfg(not(windows))]
fn send(_downs: &[KeyEvent], _ups: &[KeyEvent]) -> Result<(), ActionError> {
    Err(ActionError::UnsupportedPlatform)
}

#[cfg(windows)]
fn send(downs: &[KeyEvent], ups: &[KeyEvent]) -> Result<(), ActionError> {
    use crate::elevation::{foreground_elevated, self_elevated};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, VIRTUAL_KEY,
    };

    // Windows silently drops input aimed at an elevated window from a non-elevated sender (UIPI).
    if !self_elevated() && foreground_elevated() {
        return Err(ActionError::BlockedByElevatedWindow);
    }
    let inputs = |events: &[KeyEvent]| -> Vec<INPUT> {
        events
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
            .collect()
    };
    let size = std::mem::size_of::<INPUT>() as i32;
    // SAFETY: the slice holds fully initialised INPUT values and `size` is their exact size.
    let pressed = unsafe { SendInput(&inputs(downs), size) } as usize;
    // Release exactly the keys that went down, even after a partial press, so none stays stuck.
    let to_release = inputs(releases(ups, pressed));
    let released = if to_release.is_empty() {
        0
    } else {
        std::thread::sleep(KEY_HOLD);
        // SAFETY: as above.
        unsafe { SendInput(&to_release, size) as usize }
    };
    // A short count means UIPI, the lock screen or the UAC secure desktop blocked the input.
    if pressed == downs.len() && released == pressed {
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

    fn ctrl_shift_up() -> (Vec<KeyEvent>, Vec<KeyEvent>) {
        chord_events(&["ControlLeft".into(), "ShiftLeft".into(), "ArrowUp".into()]).unwrap()
    }

    #[test]
    fn presses_in_order_and_releases_in_reverse() {
        let (downs, ups) = ctrl_shift_up();
        assert_eq!(downs, vec![(0x1D, false, false), (0x2A, false, false), (0x48, true, false)]);
        assert_eq!(ups, vec![(0x48, true, true), (0x2A, false, true), (0x1D, false, true)]);
    }

    #[test]
    fn releases_exactly_the_keys_that_went_down() {
        let (_, ups) = ctrl_shift_up();
        assert_eq!(releases(&ups, 3), &ups[..]);
        // Only Ctrl and Shift went down: release Shift then Ctrl, never the unpressed ArrowUp.
        assert_eq!(releases(&ups, 2), &[(0x2A, false, true), (0x1D, false, true)]);
        assert_eq!(releases(&ups, 0), &[]);
    }

    #[cfg(not(windows))]
    #[test]
    fn valid_chord_is_unsupported_off_windows() {
        assert_eq!(send_chord(&["ControlLeft".into(), "KeyM".into()]), Err(ActionError::UnsupportedPlatform));
    }
}
