//! Desktop commands. Each logic lives in a plain async fn over `&AppState` so tests need no Tauri;
//! the `#[tauri::command]` wrappers only delegate. Errors are serde snake_case codes, never text.

use crate::actions::{self, ActionError};
use crate::config::{self, Config, ConfigError, SCHEMA_VERSION};
use crate::hub::layout_msg;
use crate::net;
use crate::state::{AppState, ObsState};
use crate::themes::{self, Theme, ThemeError, ThemeInfo, DEFAULT_THEME_ID};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;

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

pub fn runtime(state: &AppState) -> Runtime {
    Runtime {
        port: state.port.load(Ordering::Relaxed),
        icons_dir: state.data_dir.join("icons").to_string_lossy().into_owned(),
        elevated: crate::elevation::self_elevated(),
        first_run: state.startup.first_run,
        recovered_backup: state.startup.recovered_backup.clone(),
        platform: std::env::consts::OS.to_string(),
        server_error: state.startup.server_error.map(str::to_string),
    }
}

/// Validates, persists, swaps in, then tells the phone and the OBS task. The token is never taken from `config`.
pub async fn apply_config(state: &AppState, mut config: Config) -> Result<(), ConfigError> {
    config.validate()?;
    config.schema_version = SCHEMA_VERSION;
    let obs = config.settings.obs.clone();
    {
        // Held across the write so concurrent saves (and token changes) cannot interleave file and memory.
        let mut cur = state.config.write().await;
        config.settings.token = cur.settings.token.clone();
        config::save(&state.data_dir, &config).map_err(|_| ConfigError::SaveFailed)?;
        *cur = config;
    }
    // Only a real change restarts the OBS connection.
    state.obs_settings.send_if_modified(|v| {
        let changed = *v != obs;
        if changed {
            *v = obs;
        }
        changed
    });
    state.hub.send(layout_msg(state).await).await;
    Ok(())
}

pub async fn regenerate(state: &AppState) -> Result<PairingInfo, ConfigError> {
    {
        let mut cur = state.config.write().await;
        let mut next = cur.clone();
        next.settings.token = crate::auth::new_token();
        config::save(&state.data_dir, &next).map_err(|_| ConfigError::SaveFailed)?;
        *cur = next;
    }
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
pub async fn remove_theme(state: &AppState, id: &str) -> Result<(), ThemeError> {
    themes::delete(&state.data_dir, id)?;
    let was_active = {
        let mut cur = state.config.write().await;
        let active = cur.settings.theme == id;
        if active {
            let mut next = cur.clone();
            next.settings.theme = DEFAULT_THEME_ID.to_string();
            // A failed save leaves memory unchanged; resolve() already falls back for the missing file.
            if config::save(&state.data_dir, &next).is_ok() {
                *cur = next;
            }
        }
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
pub async fn save_config(state: St<'_>, config: Config) -> Result<(), ConfigError> {
    apply_config(&state, config).await
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

    #[tokio::test]
    async fn status_reports_phone_and_obs() {
        let f = fixture(Ok(()));
        assert_eq!(status(&f.state).await, Status { phone_connected: false, obs: ObsState::Disconnected });
        let _ = f.state.hub.attach().await;
        assert!(status(&f.state).await.phone_connected);
    }
}
