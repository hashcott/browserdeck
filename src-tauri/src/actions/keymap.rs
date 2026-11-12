/// Maps a `KeyboardEvent.code` to its Windows scan code (set 1).
/// Extended keys carry `0xE0` in the high byte; the low byte is the scan code itself.
#[rustfmt::skip]
pub fn scancode(code: &str) -> Option<u16> {
    Some(match code {
        "Escape" => 0x01,
        "Backquote" => 0x29,
        "Digit1" => 0x02, "Digit2" => 0x03, "Digit3" => 0x04, "Digit4" => 0x05, "Digit5" => 0x06,
        "Digit6" => 0x07, "Digit7" => 0x08, "Digit8" => 0x09, "Digit9" => 0x0A, "Digit0" => 0x0B,
        "Minus" => 0x0C, "Equal" => 0x0D, "Backspace" => 0x0E,
        "Tab" => 0x0F,
        "KeyQ" => 0x10, "KeyW" => 0x11, "KeyE" => 0x12, "KeyR" => 0x13, "KeyT" => 0x14,
        "KeyY" => 0x15, "KeyU" => 0x16, "KeyI" => 0x17, "KeyO" => 0x18, "KeyP" => 0x19,
        "BracketLeft" => 0x1A, "BracketRight" => 0x1B, "Backslash" => 0x2B,
        "KeyA" => 0x1E, "KeyS" => 0x1F, "KeyD" => 0x20, "KeyF" => 0x21, "KeyG" => 0x22,
        "KeyH" => 0x23, "KeyJ" => 0x24, "KeyK" => 0x25, "KeyL" => 0x26,
        "Semicolon" => 0x27, "Quote" => 0x28, "Enter" => 0x1C,
        "IntlBackslash" => 0x56,
        "KeyZ" => 0x2C, "KeyX" => 0x2D, "KeyC" => 0x2E, "KeyV" => 0x2F, "KeyB" => 0x30,
        "KeyN" => 0x31, "KeyM" => 0x32,
        "Comma" => 0x33, "Period" => 0x34, "Slash" => 0x35,
        "Space" => 0x39,
        "ControlLeft" => 0x1D, "ShiftLeft" => 0x2A, "AltLeft" => 0x38, "MetaLeft" => 0xE05B,
        "ControlRight" => 0xE01D, "ShiftRight" => 0x36, "AltRight" => 0xE038, "MetaRight" => 0xE05C,
        "F1" => 0x3B, "F2" => 0x3C, "F3" => 0x3D, "F4" => 0x3E, "F5" => 0x3F, "F6" => 0x40,
        "F7" => 0x41, "F8" => 0x42, "F9" => 0x43, "F10" => 0x44, "F11" => 0x57, "F12" => 0x58,
        "F13" => 0x64, "F14" => 0x65, "F15" => 0x66, "F16" => 0x67, "F17" => 0x68, "F18" => 0x69,
        "F19" => 0x6A, "F20" => 0x6B, "F21" => 0x6C, "F22" => 0x6D, "F23" => 0x6E, "F24" => 0x76,
        "ArrowUp" => 0xE048, "ArrowLeft" => 0xE04B, "ArrowRight" => 0xE04D, "ArrowDown" => 0xE050,
        "Insert" => 0xE052, "Delete" => 0xE053, "Home" => 0xE047, "End" => 0xE04F,
        "PageUp" => 0xE049, "PageDown" => 0xE051,
        "Numpad0" => 0x52, "Numpad1" => 0x4F, "Numpad2" => 0x50, "Numpad3" => 0x51, "Numpad4" => 0x4B,
        "Numpad5" => 0x4C, "Numpad6" => 0x4D, "Numpad7" => 0x47, "Numpad8" => 0x48, "Numpad9" => 0x49,
        "NumpadDecimal" => 0x53, "NumpadAdd" => 0x4E, "NumpadSubtract" => 0x4A,
        "NumpadMultiply" => 0x37, "NumpadDivide" => 0xE035, "NumpadEnter" => 0xE01C,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_keys() {
        assert_eq!(scancode("KeyM"), Some(0x32));
        assert_eq!(scancode("ControlLeft"), Some(0x1D));
        assert_eq!(scancode("ShiftLeft"), Some(0x2A));
        assert_eq!(scancode("F5"), Some(0x3F));
        assert_eq!(scancode("Digit1"), Some(0x02));
    }

    #[test]
    fn extended_keys_carry_e0_prefix() {
        assert_eq!(scancode("ArrowUp"), Some(0xE048));
        assert_eq!(scancode("ControlRight"), Some(0xE01D));
        assert_eq!(scancode("Delete"), Some(0xE053));
    }

    #[test]
    fn unknown_code_is_none() {
        assert_eq!(scancode("Nope"), None);
    }
}
