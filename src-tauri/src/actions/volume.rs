use super::ActionError;
use crate::config::VolumeOp;

/// Change per Up/Down press, as a fraction of full volume.
pub const VOLUME_STEP: f32 = 0.05;

/// Level after one Up/Down step, kept within 0.0..=1.0; `Mute` leaves the level as is.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
fn stepped(level: f32, op: VolumeOp) -> f32 {
    let delta = match op {
        VolumeOp::Up => VOLUME_STEP,
        VolumeOp::Down => -VOLUME_STEP,
        VolumeOp::Mute => 0.0,
    };
    (level + delta).clamp(0.0, 1.0)
}

/// Steps the default playback device's volume, or toggles its mute.
#[cfg(windows)]
pub fn volume(op: VolumeOp) -> Result<(), ActionError> {
    windows_volume(op).map_err(|_| ActionError::VolumeFailed)
}

#[cfg(windows)]
fn windows_volume(op: VolumeOp) -> windows::core::Result<()> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, COINIT_MULTITHREADED};

    // Declared first so it drops last, after every COM interface below is released.
    let _com = super::ComGuard::init(COINIT_MULTITHREADED);
    // SAFETY: COM is initialised on this thread and each call gets a live interface from the one before.
    let endpoint: IAudioEndpointVolume = unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        devices.GetDefaultAudioEndpoint(eRender, eConsole)?.Activate(CLSCTX_ALL, None)?
    };
    let no_context = std::ptr::null();
    match op {
        // SAFETY: `endpoint` is a live interface and a null event-context GUID is allowed.
        VolumeOp::Mute => unsafe { endpoint.SetMute(!endpoint.GetMute()?.as_bool(), no_context) },
        _ => {
            // SAFETY: as above.
            unsafe { endpoint.SetMasterVolumeLevelScalar(stepped(endpoint.GetMasterVolumeLevelScalar()?, op), no_context) }?;
            if op == VolumeOp::Up {
                // Like the keyboard's volume-up key, raising the volume also unmutes.
                // SAFETY: as above.
                unsafe { endpoint.SetMute(false, no_context) }?;
            }
            Ok(())
        }
    }
}

/// Steps the output volume (0–100 in AppleScript), or toggles its mute.
#[cfg(target_os = "macos")]
pub fn volume(op: VolumeOp) -> Result<(), ActionError> {
    let script = match op {
        VolumeOp::Mute => "set volume output muted not (output muted of (get volume settings))".to_string(),
        _ => {
            // "missing value" (device without volume control) fails to parse, which is the right error.
            let now: f32 = osascript("output volume of (get volume settings)")?.parse().map_err(|_| ActionError::VolumeFailed)?;
            let unmute = if op == VolumeOp::Up { " without output muted" } else { "" };
            format!("set volume output volume {}{unmute}", (stepped(now / 100.0, op) * 100.0).round() as u8)
        }
    };
    osascript(&script).map(drop)
}

#[cfg(target_os = "macos")]
fn osascript(script: &str) -> Result<String, ActionError> {
    let out = std::process::Command::new("osascript").args(["-e", script]).output();
    match out {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
        _ => Err(ActionError::VolumeFailed),
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn volume(_: VolumeOp) -> Result<(), ActionError> {
    Err(ActionError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_by_volume_step_and_clamps() {
        assert!((stepped(0.5, VolumeOp::Up) - 0.55).abs() < 1e-6);
        assert!((stepped(0.5, VolumeOp::Down) - 0.45).abs() < 1e-6);
        assert_eq!(stepped(0.98, VolumeOp::Up), 1.0);
        assert_eq!(stepped(0.02, VolumeOp::Down), 0.0);
        assert_eq!(stepped(0.3, VolumeOp::Mute), 0.3);
    }
}
