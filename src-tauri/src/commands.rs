//! Desktop commands. Each logic lives in a plain async fn over `&AppState` so tests need no Tauri;
//! the `#[tauri::command]` wrappers only delegate. Errors are serde snake_case codes, never text.

use crate::actions::{self, ActionError};
use crate::config::{self, Config, ConfigError, Settings, SCHEMA_VERSION};
use crate::hub::layout_msg;
use crate::net;
use crate::state::{AppState, ObsState};
use crate::themes::{self, Theme, ThemeError, ThemeInfo, DEFAULT_THEME_ID};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, State};

pub const MAX_ICON_BYTES: u64 = 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Runtime {
    pub port: u16,
    pub icons_dir: String,
    pub elevated: bool,
    pub first_run: bool,
    pub recovered_backup: Option<String>,
    /// std::env::consts::OS
    pub platform: String,
    /// Short code when the phone server could not bind any port.
    pub server_error: Option<String>,
    /// The last attempt to register start-with-Windows failed (at startup or from Settings).
    pub autostart_error: Option<SystemSettingError>,
    /// Why administrator mode is on but the app is not elevated, when known.
    pub admin_error: Option<SystemSettingError>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PairingAddress {
    pub ip: String,
    /// http://<ip>:<port>/?token=<token>
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PairingInfo {
    pub addresses: Vec<PairingAddress>,
    pub selected: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Status {
    pub phone_connected: bool,
    pub obs: ObsState,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IconError {
    TooLarge,
    BadType,
    Io,
}

/// Errors of the administrator-mode and start-with-Windows commands.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SystemSettingError {
    SaveFailed,
    /// The UAC prompt was refused, or the elevated copy could not be started.
    UacDeclined,
    UnsupportedPlatform,
    AutostartFailed,
    /// Administrator mode needs the app installed for all users, in Program Files.
    NeedsMachineInstall,
}

pub fn runtime(state: &AppState) -> Runtime {
    Runtime {
        port: state.port.load(Ordering::Relaxed),
        icons_dir: state.data_dir.join("icons").to_string_lossy().into_owned(),
        elevated: crate::elevation::self_elevated(),
        first_run: state.startup.first_run,
        recovered_backup: state.startup.recovered_backup.clone(),
        platform: std::env::consts::OS.to_string(),
        server_error: state.startup.server_error.map(str::to_string),
        autostart_error: *state.autostart_error.lock().unwrap_or_else(|e| e.into_inner()),
        admin_error: state.startup.admin_error,
    }
}

/// Validates, persists, swaps in, then tells the phone and the OBS task. The token and the settings only their own
/// commands change (administrator mode, start with Windows, tray notice) are never taken from `config`: a desktop
/// save sends a whole config that may predate those commands.
pub async fn apply_config(state: &AppState, mut config: Config) -> Result<(), ConfigError> {
    config.validate()?;
    config.schema_version = SCHEMA_VERSION;
    {
        // Held across the write so concurrent saves (and token changes) cannot interleave file and memory.
        let mut cur = state.config.write().await;
        let (next, now) = (&mut config.settings, &cur.settings);
        next.token = now.token.clone();
        (next.run_as_admin, next.autostart, next.tray_notice_shown) = (now.run_as_admin, now.autostart, now.tray_notice_shown);
        config::save(&state.data_dir, &config).map_err(|_| ConfigError::SaveFailed)?;
        *cur = config;
        // Inside the lock so the channel always ends at the last saved value. Only a real change
        // restarts the OBS connection.
        let obs = cur.settings.obs.clone();
        state.obs_settings.send_if_modified(|v| {
            let changed = *v != obs;
            if changed {
                *v = obs;
            }
            changed
        });
    }
    state.hub.send(layout_msg(state).await).await;
    Ok(())
}

/// Changes core-owned settings (see `apply_config`) under the config lock. Nothing the phone shows depends on them.
pub async fn update_settings(state: &AppState, f: impl FnOnce(&mut Settings)) -> Result<(), ConfigError> {
    let mut cur = state.config.write().await;
    let mut next = cur.clone();
    f(&mut next.settings);
    config::save(&state.data_dir, &next).map_err(|_| ConfigError::SaveFailed)?;
    *cur = next;
    Ok(())
}

fn saved(r: Result<(), ConfigError>) -> Result<(), SystemSettingError> {
    r.map_err(|_| SystemSettingError::SaveFailed)
}

/// Keeps how an attempt to register start-with-Windows ended, for `Runtime`.
fn applied(state: &AppState, r: Result<(), SystemSettingError>) -> Result<(), SystemSettingError> {
    *state.autostart_error.lock().unwrap_or_else(|e| e.into_inner()) = r.err();
    r
}

/// Turning administrator mode on needs a machine install (otherwise nothing is saved). The setting is saved first.
/// Turning it on unelevated starts an elevated copy through `relaunch` (UAC) and returns Ok(true): the caller must
/// exit. A refused prompt saves `false` again. Otherwise (already elevated, or turning it off) the start-with-Windows
/// entry switches now via `apply_autostart(autostart, run_as_admin)`; turning it off leaves this process elevated
/// until the app is opened again.
pub async fn admin_mode(
    state: &AppState,
    enabled: bool,
    elevated: bool,
    machine_install: bool,
    relaunch: impl FnOnce() -> std::io::Result<()>,
    apply_autostart: impl FnOnce(bool, bool) -> Result<(), SystemSettingError>,
) -> Result<bool, SystemSettingError> {
    if enabled && !machine_install {
        return Err(SystemSettingError::NeedsMachineInstall);
    }
    saved(update_settings(state, |s| s.run_as_admin = enabled).await)?;
    if enabled && !elevated {
        if relaunch().is_ok() {
            return Ok(true);
        }
        saved(update_settings(state, |s| s.run_as_admin = false).await)?;
        return Err(SystemSettingError::UacDeclined);
    }
    let autostart = state.config.read().await.settings.autostart;
    applied(state, apply_autostart(autostart, enabled))?;
    Ok(false)
}

/// Registers (or removes) the start-with-Windows entry first, and saves the setting only once that worked.
pub async fn autostart_setting(
    state: &AppState,
    enabled: bool,
    apply_autostart: impl FnOnce(bool, bool) -> Result<(), SystemSettingError>,
) -> Result<(), SystemSettingError> {
    let run_as_admin = state.config.read().await.settings.run_as_admin;
    applied(state, apply_autostart(enabled, run_as_admin))?;
    saved(update_settings(state, |s| s.autostart = enabled).await)
}

pub async fn regenerate(state: &AppState) -> Result<PairingInfo, ConfigError> {
    update_settings(state, |s| s.token = crate::auth::new_token()).await?;
    state.hub.kick().await;
    Ok(pairing(state).await)
}

/// Pure: loopback, link-local and unspecified addresses are never offered.
pub fn build_pairing_info(ips: &[Ipv4Addr], port: u16, token: &str, preferred: Option<&str>) -> PairingInfo {
    let addresses: Vec<_> = ips
        .iter()
        .filter(|ip| !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified())
        .map(|ip| PairingAddress { ip: ip.to_string(), url: format!("http://{ip}:{port}/?token={token}") })
        .collect();
    let selected = preferred
        .filter(|p| addresses.iter().any(|a| a.ip == *p))
        .or(addresses.first().map(|a| a.ip.as_str()))
        .map(str::to_string);
    PairingInfo { addresses, selected }
}

pub async fn pairing(state: &AppState) -> PairingInfo {
    let c = state.config.read().await;
    let ips = net::lan_ips();
    build_pairing_info(&ips, state.port.load(Ordering::Relaxed), &c.settings.token, c.settings.preferred_ip.as_deref())
}

pub async fn status(state: &AppState) -> Status {
    Status { phone_connected: state.hub.connected().await, obs: state.obs.state() }
}

fn magic_matches(ext: &str, head: &[u8]) -> bool {
    match ext {
        "png" => head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "jpg" | "jpeg" => head.starts_with(&[0xFF, 0xD8, 0xFF]),
        "webp" => head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP",
        _ => false,
    }
}

/// Copies an image into `data_dir/icons/<uuid>.<ext>` and returns that file name.
pub async fn import_icon_file(state: &AppState, source: &Path) -> Result<String, IconError> {
    let ext = source.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).ok_or(IconError::BadType)?;
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return Err(IconError::BadType);
    }
    let len = tokio::fs::metadata(source).await.map_err(|_| IconError::Io)?.len();
    if len > MAX_ICON_BYTES {
        return Err(IconError::TooLarge);
    }
    let bytes = tokio::fs::read(source).await.map_err(|_| IconError::Io)?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return Err(IconError::TooLarge); // grew after the metadata check
    }
    if !magic_matches(&ext, &bytes) {
        return Err(IconError::BadType);
    }
    let dir = state.data_dir.join("icons");
    tokio::fs::create_dir_all(&dir).await.map_err(|_| IconError::Io)?;
    let name = format!("{}.{ext}", uuid::Uuid::new_v4());
    tokio::fs::write(dir.join(&name), bytes).await.map_err(|_| IconError::Io)?;
    Ok(name)
}

/// Deleting the active theme falls back to the default and pushes a fresh layout.
/// Under the config lock: persist the fallback first (a failed save aborts, file kept), then delete.
pub async fn remove_theme(state: &AppState, id: &str) -> Result<(), ThemeError> {
    let was_active = {
        let mut cur = state.config.write().await;
        if themes::list(&state.data_dir).iter().any(|t| t.id == id && t.builtin) {
            return Err(ThemeError::BuiltinId);
        }
        let active = cur.settings.theme == id;
        if active {
            let mut next = cur.clone();
            next.settings.theme = DEFAULT_THEME_ID.to_string();
            config::save(&state.data_dir, &next).map_err(|_| ThemeError::Io)?;
            *cur = next;
        }
        themes::delete(&state.data_dir, id)?;
        active
    };
    if was_active {
        state.hub.send(layout_msg(state).await).await;
    }
    Ok(())
}

type St<'a> = State<'a, Arc<AppState>>;

#[tauri::command]
pub async fn get_config(state: St<'_>) -> Result<Config, ()> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
pub async fn save_config(app: AppHandle, state: St<'_>, config: Config) -> Result<(), ConfigError> {
    let language = config.settings.language.clone();
    let relabel = state.config.read().await.settings.language != language;
    apply_config(&state, config).await?;
    if relabel {
        crate::tray::relabel(&app, &language);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_runtime(state: St<'_>) -> Result<Runtime, ()> {
    Ok(runtime(&state))
}

#[tauri::command]
pub async fn pairing_info(state: St<'_>) -> Result<PairingInfo, ()> {
    Ok(pairing(&state).await)
}

#[tauri::command]
pub async fn regenerate_token(state: St<'_>) -> Result<PairingInfo, ConfigError> {
    regenerate(&state).await
}

#[tauri::command]
pub async fn test_button(state: St<'_>, button_id: String) -> Result<(), ActionError> {
    actions::press(&state, &button_id).await
}

#[tauri::command]
pub async fn obs_scenes(state: St<'_>) -> Result<Vec<String>, ActionError> {
    state.obs.scenes().await
}

#[tauri::command]
pub async fn import_icon(state: St<'_>, path: String) -> Result<String, IconError> {
    import_icon_file(&state, Path::new(&path)).await
}

#[tauri::command]
pub async fn list_themes(state: St<'_>) -> Result<Vec<ThemeInfo>, ()> {
    Ok(themes::list(&state.data_dir))
}

#[tauri::command]
pub async fn get_theme(state: St<'_>, id: String) -> Result<Theme, ()> {
    Ok(themes::resolve(&state.data_dir, &id))
}

#[tauri::command]
pub async fn import_theme(state: St<'_>, path: String) -> Result<ThemeInfo, ThemeError> {
    themes::import(&state.data_dir, Path::new(&path))
}

#[tauri::command]
pub async fn delete_theme(state: St<'_>, id: String) -> Result<(), ThemeError> {
    remove_theme(&state, &id).await
}

#[tauri::command]
pub async fn get_status(state: St<'_>) -> Result<Status, ()> {
    Ok(status(&state).await)
}

/// SECURITY: with administrator mode on, the elevated app runs whatever `path` + `args` the buttons in
/// config.json name, and config.json lives in the user's own, user-writable app data folder. So any program
/// running as this user can rewrite a button and have it run as administrator, which is a way to gain admin
/// rights without a UAC prompt. The pairing token sits in the same file, so such a program need not even wait for
/// someone to press the button: it can press it itself over the phone server. Not addressed here (no ACL changes,
/// no de-elevated launch); see the Task 11 report. (The exe itself is covered: administrator mode requires a
/// machine install, see `elevation::machine_install`.)
#[tauri::command]
pub async fn set_admin_mode(app: AppHandle, state: St<'_>, enabled: bool) -> Result<(), SystemSettingError> {
    if !cfg!(windows) {
        return Err(SystemSettingError::UnsupportedPlatform);
    }
    // The elevated copy waits for this process to end, so it gets the phone port and the single-instance lock.
    let wait = format!("{}{}", crate::elevation::WAIT_PID_ARG, std::process::id());
    let relaunch = || crate::elevation::relaunch_elevated(&[&wait]);
    let apply = |on, admin| crate::autostart::apply(&app, on, admin);
    let (elevated, machine) = (crate::elevation::self_elevated(), crate::elevation::machine_install());
    if admin_mode(&state, enabled, elevated, machine, relaunch, apply).await? {
        app.exit(0);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_autostart(app: AppHandle, state: St<'_>, enabled: bool) -> Result<(), SystemSettingError> {
    autostart_setting(&state, enabled, |on, admin| crate::autostart::apply(&app, on, admin)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::testing::{fixture, Fixture};
    use crate::config::is_icon_file_name;
    use crate::hub::ServerMsg;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    async fn cfg(f: &Fixture) -> Config {
        f.state.config.read().await.clone()
    }

    fn on_disk(f: &Fixture) -> String {
        std::fs::read_to_string(f.state.data_dir.join("config.json")).unwrap_or_default()
    }

    #[tokio::test]
    async fn save_config_keeps_existing_token() {
        let f = fixture(Ok(()));
        let mut c = cfg(&f).await;
        c.settings.token = "evil".into();
        c.settings.language = "vi".into();
        apply_config(&f.state, c).await.unwrap();
        let now = cfg(&f).await;
        assert_eq!((now.settings.token.as_str(), now.settings.language.as_str()), ("secret", "vi"));
        let saved: Config = serde_json::from_str(&on_disk(&f)).unwrap();
        assert_eq!(saved, now);
    }

    #[tokio::test]
    async fn save_config_rejects_invalid_and_changes_nothing() {
        let f = fixture(Ok(()));
        apply_config(&f.state, cfg(&f).await).await.unwrap();
        let (before_disk, before) = (on_disk(&f), cfg(&f).await);
        let mut c = before.clone();
        c.pages[0].buttons[1].slot = 0;
        assert_eq!(apply_config(&f.state, c).await, Err(ConfigError::DuplicateSlot));
        let mut c = before.clone();
        c.settings.port = 0;
        assert_eq!(apply_config(&f.state, c).await, Err(ConfigError::BadPort));
        assert_eq!((on_disk(&f), cfg(&f).await), (before_disk, before));
    }

    #[tokio::test]
    async fn save_config_pushes_layout_to_phone() {
        let f = fixture(Ok(()));
        let (_id, mut rx) = f.state.hub.attach().await;
        let mut c = cfg(&f).await;
        let mut page = c.pages[0].clone();
        page.id = "second".into();
        page.name = "Second".into();
        page.buttons.clear();
        c.pages.push(page);
        apply_config(&f.state, c).await.unwrap();
        match rx.recv().await {
            Some(ServerMsg::Layout { pages, .. }) => assert_eq!(pages.len(), 2),
            other => panic!("expected layout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn save_config_only_notifies_obs_on_change() {
        let f = fixture(Ok(()));
        let mut rx = f.state.obs_settings.subscribe();
        rx.borrow_and_update();
        apply_config(&f.state, cfg(&f).await).await.unwrap();
        assert!(!rx.has_changed().unwrap());
        let mut c = cfg(&f).await;
        c.settings.obs.password = "pw".into();
        apply_config(&f.state, c).await.unwrap();
        assert_eq!(rx.borrow_and_update().password, "pw");
    }

    #[tokio::test]
    async fn regenerate_token_changes_token_and_kicks_phone() {
        let f = fixture(Ok(()));
        let (_id, mut rx) = f.state.hub.attach().await;
        let info = regenerate(&f.state).await.unwrap();
        let token = cfg(&f).await.settings.token;
        assert_ne!(token, "secret");
        assert!(on_disk(&f).contains(&token));
        assert_eq!(rx.recv().await, Some(ServerMsg::Kicked));
        assert!(info.addresses.iter().all(|a| a.url.ends_with(&format!("/?token={token}"))));
    }

    #[tokio::test]
    async fn pairing_info_uses_real_port_and_token() {
        let f = fixture(Ok(()));
        f.state.port.store(47825, Ordering::Relaxed);
        let info = build_pairing_info(&[ip("192.168.1.20")], 47825, "tok", None);
        assert_eq!(info.addresses[0].url, "http://192.168.1.20:47825/?token=tok");
        assert_eq!(info.selected.as_deref(), Some("192.168.1.20"));
        for a in pairing(&f.state).await.addresses {
            assert!(a.url.ends_with(":47825/?token=secret"), "{}", a.url);
        }
    }

    #[test]
    fn pairing_info_prefers_listed_ip_and_never_offers_loopback() {
        let ips = [ip("192.168.1.20"), ip("10.0.0.5"), ip("127.0.0.1"), ip("169.254.3.3")];
        let info = build_pairing_info(&ips, 1, "t", Some("10.0.0.5"));
        assert_eq!(info.addresses.len(), 2);
        assert_eq!(info.selected.as_deref(), Some("10.0.0.5"));
        assert_eq!(build_pairing_info(&ips, 1, "t", Some("10.9.9.9")).selected.as_deref(), Some("192.168.1.20"));
        assert_eq!(build_pairing_info(&ips, 1, "t", Some("127.0.0.1")).selected.as_deref(), Some("192.168.1.20"));
    }

    #[test]
    fn pairing_info_with_no_lan_has_no_selection() {
        let info = build_pairing_info(&[], 47821, "t", None);
        assert!(info.addresses.is_empty());
        assert_eq!(info.selected, None);
        assert_eq!(build_pairing_info(&[], 47821, "t", Some("10.0.0.5")).selected, None);
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];

    #[tokio::test]
    async fn import_icon_rejects_svg_and_oversize() {
        let f = fixture(Ok(()));
        let d = tempfile::tempdir().unwrap();
        let svg = d.path().join("a.svg");
        std::fs::write(&svg, "<svg/>").unwrap();
        assert_eq!(import_icon_file(&f.state, &svg).await, Err(IconError::BadType));
        let big = d.path().join("big.png");
        let mut bytes = PNG.to_vec();
        bytes.resize(MAX_ICON_BYTES as usize + 1, 0);
        std::fs::write(&big, bytes).unwrap();
        assert_eq!(import_icon_file(&f.state, &big).await, Err(IconError::TooLarge));
        assert!(!f.state.data_dir.join("icons").exists());
    }

    #[tokio::test]
    async fn import_icon_checks_magic_bytes_against_extension() {
        let f = fixture(Ok(()));
        let d = tempfile::tempdir().unwrap();
        let fake = d.path().join("evil.PNG");
        std::fs::write(&fake, b"MZ not a png").unwrap();
        assert_eq!(import_icon_file(&f.state, &fake).await, Err(IconError::BadType));
        let jpg_as_png = d.path().join("x.png");
        std::fs::write(&jpg_as_png, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        assert_eq!(import_icon_file(&f.state, &jpg_as_png).await, Err(IconError::BadType));
        let mut webp = b"RIFF\x04\0\0\0WEBP".to_vec();
        webp.extend_from_slice(b"VP8 ");
        let ok = d.path().join("w.webp");
        std::fs::write(&ok, webp).unwrap();
        assert!(import_icon_file(&f.state, &ok).await.unwrap().ends_with(".webp"));
        assert_eq!(import_icon_file(&f.state, &d.path().join("missing.png")).await, Err(IconError::Io));
    }

    #[tokio::test]
    async fn import_icon_copies_with_safe_name() {
        let f = fixture(Ok(()));
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("My Icon.PNG");
        std::fs::write(&src, PNG).unwrap();
        let name = import_icon_file(&f.state, &src).await.unwrap();
        assert!(is_icon_file_name(&name), "{name}");
        assert_eq!(std::fs::read(f.state.data_dir.join("icons").join(&name)).unwrap(), PNG);
    }

    async fn next_theme(rx: &mut tokio::sync::mpsc::UnboundedReceiver<ServerMsg>) -> String {
        match rx.recv().await {
            Some(ServerMsg::Layout { theme, .. }) => theme.id,
            other => panic!("expected layout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn changing_theme_pushes_layout_with_new_theme() {
        let f = fixture(Ok(()));
        let (_id, mut rx) = f.state.hub.attach().await;
        let mut c = cfg(&f).await;
        c.settings.theme = "arcade".into();
        apply_config(&f.state, c).await.unwrap();
        assert_eq!(next_theme(&mut rx).await, "arcade");
    }

    fn theme_file(dir: &Path) -> std::path::PathBuf {
        let mut v = serde_json::to_value(themes::resolve(dir, "keycap")).unwrap();
        v["id"] = "mine".into();
        v["name"] = "<b>Mine</b>".into();
        let p = dir.join("mine-src.json");
        std::fs::write(&p, v.to_string()).unwrap();
        p
    }

    #[tokio::test]
    async fn deleting_active_theme_falls_back_to_keycap() {
        let f = fixture(Ok(()));
        let src = theme_file(&f.state.data_dir);
        let info = themes::import(&f.state.data_dir, &src).unwrap();
        assert_eq!(info.name, "<b>Mine</b>"); // passed through as data
        let mut c = cfg(&f).await;
        c.settings.theme = "mine".into();
        apply_config(&f.state, c).await.unwrap();
        let (_id, mut rx) = f.state.hub.attach().await;
        remove_theme(&f.state, "mine").await.unwrap();
        assert_eq!(cfg(&f).await.settings.theme, "keycap");
        assert!(on_disk(&f).contains("\"theme\": \"keycap\""));
        assert_eq!(next_theme(&mut rx).await, "keycap");
        assert!(themes::list(&f.state.data_dir).iter().all(|t| t.id != "mine"));
    }

    #[tokio::test]
    async fn deleting_inactive_theme_leaves_config_and_phone_alone() {
        let f = fixture(Ok(()));
        let src = theme_file(&f.state.data_dir);
        themes::import(&f.state.data_dir, &src).unwrap();
        let (_id, mut rx) = f.state.hub.attach().await;
        remove_theme(&f.state, "mine").await.unwrap();
        assert_eq!(cfg(&f).await.settings.theme, "keycap");
        assert!(rx.try_recv().is_err());
        assert_eq!(remove_theme(&f.state, "mine").await, Err(ThemeError::NotFound));
        assert_eq!(remove_theme(&f.state, "keycap").await, Err(ThemeError::BuiltinId));
    }

    /// Makes `config::save` fail: its temp file path is a directory.
    fn break_saving(f: &Fixture) {
        std::fs::create_dir(f.state.data_dir.join("config.json.tmp")).unwrap();
    }

    #[tokio::test]
    async fn save_failure_returns_save_failed_and_changes_nothing() {
        let f = fixture(Ok(()));
        let (_id, mut rx) = f.state.hub.attach().await;
        let mut obs_rx = f.state.obs_settings.subscribe();
        obs_rx.borrow_and_update();
        let before = cfg(&f).await;
        break_saving(&f);
        let mut c = before.clone();
        c.settings.language = "vi".into();
        c.settings.obs.password = "pw".into();
        assert_eq!(apply_config(&f.state, c).await, Err(ConfigError::SaveFailed));
        assert_eq!(cfg(&f).await, before);
        assert!(rx.try_recv().is_err());
        assert!(!obs_rx.has_changed().unwrap());
        assert_eq!(obs_rx.borrow().password, before.settings.obs.password);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_saves_leave_obs_channel_equal_to_config() {
        let f = fixture(Ok(()));
        let base = cfg(&f).await;
        let tasks: Vec<_> = (0..40)
            .map(|i| {
                let state = f.state.clone();
                let mut c = base.clone();
                c.settings.obs.password = format!("pw{i}");
                tokio::spawn(async move { apply_config(&state, c).await.unwrap() })
            })
            .collect();
        for t in tasks {
            t.await.unwrap();
        }
        let in_config = cfg(&f).await.settings.obs;
        assert_eq!(*f.state.obs_settings.borrow(), in_config);
    }

    #[tokio::test]
    async fn deleting_active_theme_with_unsavable_config_keeps_everything() {
        let f = fixture(Ok(()));
        let src = theme_file(&f.state.data_dir);
        themes::import(&f.state.data_dir, &src).unwrap();
        let mut c = cfg(&f).await;
        c.settings.theme = "mine".into();
        apply_config(&f.state, c).await.unwrap();
        let (_id, mut rx) = f.state.hub.attach().await;
        break_saving(&f);
        assert_eq!(remove_theme(&f.state, "mine").await, Err(ThemeError::Io));
        assert_eq!(cfg(&f).await.settings.theme, "mine");
        assert!(themes::list(&f.state.data_dir).iter().any(|t| t.id == "mine"));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn deleting_active_builtin_theme_is_refused_without_side_effects() {
        let f = fixture(Ok(()));
        let mut c = cfg(&f).await;
        c.settings.theme = "arcade".into();
        apply_config(&f.state, c).await.unwrap();
        assert_eq!(remove_theme(&f.state, "arcade").await, Err(ThemeError::BuiltinId));
        assert_eq!(cfg(&f).await.settings.theme, "arcade");
    }

    #[tokio::test]
    async fn status_reports_phone_and_obs() {
        let f = fixture(Ok(()));
        assert_eq!(status(&f.state).await, Status { phone_connected: false, obs: ObsState::Disconnected });
        let _ = f.state.hub.attach().await;
        assert!(status(&f.state).await.phone_connected);
    }

    #[tokio::test]
    async fn save_config_cannot_change_core_owned_settings() {
        let f = fixture(Ok(()));
        let mut c = cfg(&f).await;
        c.settings.run_as_admin = true;
        c.settings.autostart = true;
        c.settings.tray_notice_shown = true;
        c.settings.language = "vi".into();
        apply_config(&f.state, c).await.unwrap();
        let s = cfg(&f).await.settings;
        assert_eq!((s.run_as_admin, s.autostart, s.tray_notice_shown), (false, false, false));
        assert_eq!(s.language, "vi");
        update_settings(&f.state, |s| s.tray_notice_shown = true).await.unwrap();
        apply_config(&f.state, cfg(&f).await).await.unwrap();
        assert!(cfg(&f).await.settings.tray_notice_shown);
        assert!(on_disk(&f).contains("\"tray_notice_shown\": true"));
    }

    type Applied = Result<(), SystemSettingError>;

    fn never_apply(_: bool, _: bool) -> Applied {
        panic!("autostart must not be touched here")
    }

    #[tokio::test]
    async fn admin_mode_on_saves_before_relaunching_then_asks_to_exit() {
        let f = fixture(Ok(()));
        let disk = f.state.data_dir.clone();
        let relaunch = || {
            let saved = std::fs::read_to_string(disk.join("config.json")).unwrap();
            assert!(saved.contains("\"run_as_admin\": true"), "saved before the relaunch");
            Ok(())
        };
        assert_eq!(admin_mode(&f.state, true, false, true, relaunch, never_apply).await, Ok(true));
        assert!(cfg(&f).await.settings.run_as_admin);
    }

    #[tokio::test]
    async fn admin_mode_needs_a_machine_install_and_then_changes_nothing() {
        let f = fixture(Ok(()));
        let before = on_disk(&f);
        let no_relaunch = || -> std::io::Result<()> { panic!("no relaunch without a machine install") };
        for elevated in [false, true] {
            let r = admin_mode(&f.state, true, elevated, false, no_relaunch, never_apply).await;
            assert_eq!(r, Err(SystemSettingError::NeedsMachineInstall));
        }
        assert!(!cfg(&f).await.settings.run_as_admin);
        assert_eq!(on_disk(&f), before);
        // turning it off is always allowed
        let off = admin_mode(&f.state, false, false, false, no_relaunch, |_, _| Ok(())).await;
        assert_eq!(off, Ok(false));
    }

    #[tokio::test]
    async fn admin_mode_declined_reverts_and_reports() {
        let f = fixture(Ok(()));
        let declined = || Err(std::io::Error::from_raw_os_error(1223)); // ERROR_CANCELLED
        assert_eq!(admin_mode(&f.state, true, false, true, declined, never_apply).await, Err(SystemSettingError::UacDeclined));
        assert!(!cfg(&f).await.settings.run_as_admin);
        assert!(on_disk(&f).contains("\"run_as_admin\": false"));
    }

    #[tokio::test]
    async fn admin_mode_without_relaunch_reapplies_autostart() {
        let f = fixture(Ok(()));
        update_settings(&f.state, |s| s.autostart = true).await.unwrap();
        let no_relaunch = || -> std::io::Result<()> { panic!("already elevated or turning off: no relaunch") };
        let applied = std::sync::Mutex::new(vec![]);
        let apply = |a, r| { applied.lock().unwrap().push((a, r)); Ok(()) };
        assert_eq!(admin_mode(&f.state, true, true, true, no_relaunch, apply).await, Ok(false));
        assert!(cfg(&f).await.settings.run_as_admin);
        assert_eq!(admin_mode(&f.state, false, true, true, no_relaunch, apply).await, Ok(false));
        assert!(!cfg(&f).await.settings.run_as_admin);
        assert_eq!(*applied.lock().unwrap(), [(true, true), (true, false)]);
        let failing = |_, _| Err(SystemSettingError::AutostartFailed);
        assert_eq!(admin_mode(&f.state, false, true, true, no_relaunch, failing).await, Err(SystemSettingError::AutostartFailed));
        assert_eq!(runtime(&f.state).autostart_error, Some(SystemSettingError::AutostartFailed));
    }

    #[tokio::test]
    async fn autostart_applies_first_saves_only_on_success_and_keeps_the_outcome_for_runtime() {
        let f = fixture(Ok(()));
        update_settings(&f.state, |s| s.run_as_admin = true).await.unwrap();
        assert_eq!(runtime(&f.state).autostart_error, None);
        let failing = |_, _| Err(SystemSettingError::AutostartFailed);
        assert_eq!(autostart_setting(&f.state, true, failing).await, Err(SystemSettingError::AutostartFailed));
        assert_eq!(runtime(&f.state).autostart_error, Some(SystemSettingError::AutostartFailed));
        let not_here = |_, _| Err(SystemSettingError::NeedsMachineInstall);
        assert_eq!(autostart_setting(&f.state, true, not_here).await, Err(SystemSettingError::NeedsMachineInstall));
        assert!(!cfg(&f).await.settings.autostart);
        let seen = std::sync::Mutex::new(None);
        autostart_setting(&f.state, true, |a, r| { *seen.lock().unwrap() = Some((a, r)); Ok(()) }).await.unwrap();
        assert_eq!(*seen.lock().unwrap(), Some((true, true)));
        assert!(cfg(&f).await.settings.autostart);
        assert!(on_disk(&f).contains("\"autostart\": true"));
        assert_eq!(runtime(&f.state).autostart_error, None);
    }

    #[tokio::test]
    async fn system_settings_report_save_failure() {
        let f = fixture(Ok(()));
        break_saving(&f);
        assert_eq!(autostart_setting(&f.state, true, |_, _| Ok(())).await, Err(SystemSettingError::SaveFailed));
        assert_eq!(admin_mode(&f.state, true, false, true, || Ok(()), never_apply).await, Err(SystemSettingError::SaveFailed));
        let s = cfg(&f).await.settings;
        assert!(!s.autostart && !s.run_as_admin);
    }
}
