use super::ActionError;
use crate::config::MediaOp;

/// Sends the keyboard's play/pause, next or previous media key; whichever player holds the system's media
/// session (Spotify, a browser tab, the system player) acts on it. `BlockedByElevatedWindow`: as for a hotkey,
/// Windows drops input aimed at an elevated window from a non-elevated sender.
#[cfg(windows)]
pub fn media(op: MediaOp) -> Result<(), ActionError> {
    use crate::elevation::{foreground_elevated, self_elevated};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
    };

    if !self_elevated() && foreground_elevated() {
        return Err(ActionError::BlockedByElevatedWindow);
    }
    let vk = match op {
        MediaOp::PlayPause => VK_MEDIA_PLAY_PAUSE,
        MediaOp::Next => VK_MEDIA_NEXT_TRACK,
        MediaOp::Previous => VK_MEDIA_PREV_TRACK,
    };
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = [key(KEYEVENTF_EXTENDEDKEY), key(KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP)];
    // SAFETY: the array holds fully initialised INPUT values and the size is theirs.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize;
    // A short count means UIPI, the lock screen or the UAC secure desktop blocked the input.
    if sent == inputs.len() { Ok(()) } else { Err(ActionError::BlockedByElevatedWindow) }
}

/// The same key as the keyboard sends it: a system-defined event (subtype 8) carrying NX_KEYTYPE_PLAY (16),
/// _NEXT (17) or _PREVIOUS (18), down then up. Posting it needs the Accessibility permission.
#[cfg(target_os = "macos")]
pub fn media(op: MediaOp) -> Result<(), ActionError> {
    let key = match op {
        MediaOp::PlayPause => 16,
        MediaOp::Next => 17,
        MediaOp::Previous => 18,
    };
    let script = format!(
        r#"ObjC.import("Cocoa");
if (!$.AXIsProcessTrusted()) throw new Error("no accessibility permission");
for (const [flags, state] of [[0xa00, 0xa], [0xb00, 0xb]]) {{
  const e = $.NSEvent.otherEventWithTypeLocationModifierFlagsTimestampWindowNumberContextSubtypeData1Data2(14, $.NSMakePoint(0, 0), flags, 0, 0, null, 8, ({key} << 16) | (state << 8), -1);
  $.CGEventPost(0, e.CGEvent);
}}"#
    );
    let out = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", &script]).output();
    if out.is_ok_and(|o| o.status.success()) { Ok(()) } else { Err(ActionError::MediaFailed) }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn media(_: MediaOp) -> Result<(), ActionError> {
    Err(ActionError::UnsupportedPlatform)
}
