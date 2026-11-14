use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
const FILE: &str = "config.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Config {
    pub schema_version: u32,
    pub settings: Settings,
    pub pages: Vec<Page>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    pub token: String,
    pub port: u16,
    pub obs: ObsSettings,
    pub grid: Grid,
    /// "auto" | "en" | "vi"
    pub language: String,
    pub run_as_admin: bool,
    pub autostart: bool,
    pub tray_notice_shown: bool,
    pub preferred_ip: Option<String>,
    /// Button theme id.
    pub theme: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ObsSettings {
    pub host: String,
    pub port: u16,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub buttons: Vec<Button>,
    /// The page's own grid size; None = `settings.grid`. Absent in files written before per-page grids.
    #[serde(default)]
    pub grid: Option<Grid>,
    /// The page's own button theme id; None = `settings.theme`.
    #[serde(default)]
    pub theme: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Button {
    pub id: String,
    /// Row-major, from 0.
    pub slot: u32,
    pub label: String,
    pub icon: Icon,
    /// "#rrggbb"; None = theme colour for the action type.
    pub color: Option<String>,
    pub action: Action,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Icon {
    Emoji { value: String },
    Image { file: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Launch {
        path: String,
        args: Vec<String>,
        /// Ask Windows for administrator rights (UAC) on every press.
        #[serde(default)]
        admin: bool,
    },
    /// KeyboardEvent.code values, e.g. ["ControlLeft", "KeyM"].
    Hotkey { keys: Vec<String> },
    Volume { op: VolumeOp },
    ObsScene { scene: String },
    /// Shows a live reading on the phone; pressing it does nothing.
    Usage { metric: Metric },
    Media { op: MediaOp },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaOp {
    PlayPause,
    Next,
    Previous,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Cpu,
    Ram,
    Temp,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VolumeOp {
    Up,
    Down,
    Mute,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigError {
    NoPages,
    BadGrid,
    SlotOutOfRange,
    DuplicateSlot,
    DuplicateId,
    BadColor,
    LabelTooLong,
    BadIconFile,
    BadTheme,
    BadPort,
    SaveFailed,
}

#[derive(Debug, PartialEq)]
pub enum LoadOutcome {
    Loaded(Config),
    Created(Config),
    Recovered { config: Config, backup: PathBuf },
}

fn rand_id() -> String {
    crate::auth::new_token()[..16].to_string()
}

impl Config {
    pub fn default_with_token(token: String) -> Config {
        let volume = |slot, op, emoji: &str| Button {
            id: rand_id(),
            slot,
            label: String::new(),
            icon: Icon::Emoji { value: emoji.into() },
            color: None,
            action: Action::Volume { op },
        };
        Config {
            schema_version: SCHEMA_VERSION,
            settings: Settings {
                token,
                port: 47821,
                obs: ObsSettings { host: "127.0.0.1".into(), port: 4455, password: String::new() },
                grid: Grid { cols: 3, rows: 4 },
                language: "auto".into(),
                run_as_admin: false,
                autostart: false,
                tray_notice_shown: false,
                preferred_ip: None,
                theme: "keycap".into(),
            },
            pages: vec![Page {
                id: rand_id(),
                name: "1".into(),
                buttons: vec![
                    volume(0, VolumeOp::Down, "\u{1F509}"),
                    volume(1, VolumeOp::Up, "\u{1F50A}"),
                    volume(2, VolumeOp::Mute, "\u{1F507}"),
                ],
                grid: None,
                theme: None,
            }],
        }
    }

    /// The grid a page is laid out on: its own, or the default.
    pub fn page_grid<'a>(&'a self, page: &'a Page) -> &'a Grid {
        page.grid.as_ref().unwrap_or(&self.settings.grid)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        use ConfigError::*;
        let grid_ok = |g: &Grid| (1..=8).contains(&g.cols) && (1..=8).contains(&g.rows);
        let theme_ok = |t: &str| {
            (1..=40).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        if !grid_ok(&self.settings.grid) {
            return Err(BadGrid);
        }
        if self.settings.port == 0 {
            return Err(BadPort);
        }
        if !theme_ok(&self.settings.theme) {
            return Err(BadTheme);
        }
        if self.pages.is_empty() {
            return Err(NoPages);
        }
        let mut ids = HashSet::new();
        for p in &self.pages {
            if !ids.insert(p.id.as_str()) {
                return Err(DuplicateId);
            }
            if p.grid.as_ref().is_some_and(|g| !grid_ok(g)) {
                return Err(BadGrid);
            }
            if p.theme.as_deref().is_some_and(|t| !theme_ok(t)) {
                return Err(BadTheme);
            }
            let g = self.page_grid(p);
            let mut slots = HashSet::new();
            for b in &p.buttons {
                if !ids.insert(b.id.as_str()) {
                    return Err(DuplicateId);
                }
                if b.slot >= g.cols * g.rows {
                    return Err(SlotOutOfRange);
                }
                if !slots.insert(b.slot) {
                    return Err(DuplicateSlot);
                }
                if b.label.chars().count() > 40 {
                    return Err(LabelTooLong);
                }
                if let Some(c) = &b.color {
                    if !is_hex_color(c) {
                        return Err(BadColor);
                    }
                }
                if let Icon::Image { file } = &b.icon {
                    if !is_icon_file_name(file) {
                        return Err(BadIconFile);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn find_button(&self, id: &str) -> Option<&Button> {
        self.pages.iter().flat_map(|p| &p.buttons).find(|b| b.id == id)
    }
}

/// `^#[0-9a-fA-F]{6}$`
pub(crate) fn is_hex_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|x| x.is_ascii_hexdigit())
}

/// `^[0-9a-f-]{36}\.(png|jpg|jpeg|webp)$`
pub fn is_icon_file_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else { return false };
    stem.len() == 36
        && stem.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'-'))
        && matches!(ext, "png" | "jpg" | "jpeg" | "webp")
}

pub fn save(dir: &Path, config: &Config) -> io::Result<()> {
    let tmp = dir.join("config.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(config)?)?;
    std::fs::rename(&tmp, dir.join(FILE))
}

/// `run_as_admin` as `load_or_init` would load it, without creating or repairing anything: false whenever the
/// file would not load as it is.
pub fn peek_run_as_admin(dir: &Path) -> bool {
    std::fs::read(dir.join(FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Config>(&bytes).ok())
        .is_some_and(|c| c.schema_version == SCHEMA_VERSION && c.validate().is_ok() && c.settings.run_as_admin)
}

pub fn load_or_init(dir: &Path) -> io::Result<LoadOutcome> {
    let path = dir.join(FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let c = Config::default_with_token(crate::auth::new_token());
            save(dir, &c)?;
            return Ok(LoadOutcome::Created(c));
        }
        // Non-UTF-8 content is corrupt, not an IO failure: back it up below.
        Err(e) if e.kind() == io::ErrorKind::InvalidData => String::new(),
        Err(e) => return Err(e),
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(c) if c.schema_version == SCHEMA_VERSION && c.validate().is_ok() => Ok(LoadOutcome::Loaded(c)),
        _ => {
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let backup = dir.join(format!("config.corrupt-{secs}.json"));
            std::fs::rename(&path, &backup)?;
            let c = Config::default_with_token(crate::auth::new_token());
            save(dir, &c)?;
            Ok(LoadOutcome::Recovered { config: c, backup })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default_with_token("t".into())
    }

    #[test]
    fn default_config_is_valid_and_has_three_volume_buttons() {
        let c = cfg();
        assert_eq!(c.validate(), Ok(()));
        assert_eq!(c.settings.port, 47821);
        assert_eq!((c.settings.grid.cols, c.settings.grid.rows), (3, 4));
        assert_eq!(c.settings.theme, "keycap");
        assert_eq!(c.pages[0].buttons.len(), 3);
        assert!(c.pages[0]
            .buttons
            .iter()
            .all(|b| matches!(b.action, Action::Volume { .. }) && b.label.is_empty()));
    }

    #[test]
    fn validate_rejects_slot_outside_grid() {
        let mut c = cfg();
        c.pages[0].buttons[0].slot = 12;
        assert_eq!(c.validate(), Err(ConfigError::SlotOutOfRange));
    }

    #[test]
    fn a_page_grid_sets_its_own_slot_range() {
        let mut c = cfg(); // default grid 3 x 4 = 12 slots
        c.pages[0].grid = Some(Grid { cols: 4, rows: 4 });
        c.pages[0].buttons[0].slot = 15;
        assert_eq!(c.validate(), Ok(()));
        c.pages[0].grid = Some(Grid { cols: 2, rows: 2 });
        assert_eq!(c.validate(), Err(ConfigError::SlotOutOfRange));
        c.pages[0].grid = Some(Grid { cols: 9, rows: 2 });
        assert_eq!(c.validate(), Err(ConfigError::BadGrid));
        c.pages[0].grid = None; // back on the default grid, slot 15 is outside it
        assert_eq!(c.validate(), Err(ConfigError::SlotOutOfRange));
    }

    #[test]
    fn a_page_theme_is_checked_like_the_default() {
        let mut c = cfg();
        c.pages[0].theme = Some("arcade".into());
        assert_eq!(c.validate(), Ok(()));
        c.pages[0].theme = Some("../x".into());
        assert_eq!(c.validate(), Err(ConfigError::BadTheme));
    }

    #[test]
    fn a_file_without_page_grid_or_theme_still_loads() {
        let mut v = serde_json::to_value(cfg()).unwrap();
        let page = v["pages"][0].as_object_mut().unwrap();
        page.remove("grid");
        page.remove("theme");
        let c: Config = serde_json::from_value(v).unwrap();
        assert_eq!((c.pages[0].grid.clone(), c.pages[0].theme.clone()), (None, None));
    }

    #[test]
    fn validate_rejects_duplicate_slot() {
        let mut c = cfg();
        c.pages[0].buttons[1].slot = 0;
        assert_eq!(c.validate(), Err(ConfigError::DuplicateSlot));
    }

    #[test]
    fn validate_rejects_bad_color() {
        let mut c = cfg();
        c.pages[0].buttons[0].color = Some("red".into());
        assert_eq!(c.validate(), Err(ConfigError::BadColor));
        c.pages[0].buttons[0].color = Some("#aBc012".into());
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_traversal_icon_name() {
        let mut c = cfg();
        c.pages[0].buttons[0].icon = Icon::Image { file: "../x.png".into() };
        assert_eq!(c.validate(), Err(ConfigError::BadIconFile));
        assert!(!is_icon_file_name("not-a-uuid.png"));
        assert!(is_icon_file_name("01234567-89ab-cdef-0123-456789abcdef.webp"));
    }

    #[test]
    fn validate_rejects_bad_theme() {
        let mut c = cfg();
        c.settings.theme = "Bad Theme".into();
        assert_eq!(c.validate(), Err(ConfigError::BadTheme));
        c.settings.theme = String::new();
        assert_eq!(c.validate(), Err(ConfigError::BadTheme));
    }

    #[test]
    fn validate_rejects_structural_errors() {
        let mut c = cfg();
        c.pages.clear();
        assert_eq!(c.validate(), Err(ConfigError::NoPages));
        let mut c = cfg();
        c.settings.grid.cols = 9;
        assert_eq!(c.validate(), Err(ConfigError::BadGrid));
        let mut c = cfg();
        c.pages[0].buttons[1].id = c.pages[0].buttons[0].id.clone();
        assert_eq!(c.validate(), Err(ConfigError::DuplicateId));
        let mut c = cfg();
        c.pages[0].buttons[0].label = "x".repeat(41);
        assert_eq!(c.validate(), Err(ConfigError::LabelTooLong));
    }

    #[test]
    fn find_button_by_id() {
        let c = cfg();
        let id = c.pages[0].buttons[2].id.clone();
        assert_eq!(c.find_button(&id).unwrap().slot, 2);
        assert!(c.find_button("nope").is_none());
    }

    #[test]
    fn launch_without_admin_field_loads_as_not_admin() {
        let a: Action = serde_json::from_str(r#"{"type":"launch","path":"C:\\a.exe","args":[]}"#).unwrap();
        assert_eq!(a, Action::Launch { path: r"C:\a.exe".into(), args: vec![], admin: false });
    }

    #[test]
    fn save_then_load_round_trips() {
        let d = tempfile::tempdir().unwrap();
        let c = cfg();
        save(d.path(), &c).unwrap();
        assert_eq!(load_or_init(d.path()).unwrap(), LoadOutcome::Loaded(c));
    }

    #[test]
    fn first_run_creates_file() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(load_or_init(d.path()).unwrap(), LoadOutcome::Created(_)));
        assert!(d.path().join("config.json").exists());
    }

    #[test]
    fn corrupt_file_is_backed_up_not_lost() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("config.json"), "{not json").unwrap();
        match load_or_init(d.path()).unwrap() {
            LoadOutcome::Recovered { backup, .. } => {
                assert_eq!(std::fs::read_to_string(backup).unwrap(), "{not json");
            }
            o => panic!("expected Recovered, got {o:?}"),
        }
        assert!(matches!(load_or_init(d.path()).unwrap(), LoadOutcome::Loaded(_)));
    }

    #[test]
    fn unknown_schema_version_is_backed_up() {
        let d = tempfile::tempdir().unwrap();
        let mut c = cfg();
        c.schema_version = 99;
        save(d.path(), &c).unwrap();
        assert!(matches!(load_or_init(d.path()).unwrap(), LoadOutcome::Recovered { .. }));
    }

    #[test]
    fn invalid_config_is_backed_up() {
        let d = tempfile::tempdir().unwrap();
        let mut c = cfg();
        c.pages.clear();
        save(d.path(), &c).unwrap();
        assert!(matches!(load_or_init(d.path()).unwrap(), LoadOutcome::Recovered { .. }));
    }

    #[test]
    fn peek_run_as_admin_only_trusts_a_config_that_would_load() {
        let d = tempfile::tempdir().unwrap();
        assert!(!peek_run_as_admin(d.path()));
        let mut c = cfg();
        c.settings.run_as_admin = true;
        save(d.path(), &c).unwrap();
        assert!(peek_run_as_admin(d.path()));
        c.settings.grid.cols = 0;
        save(d.path(), &c).unwrap();
        assert!(!peek_run_as_admin(d.path()));
        std::fs::write(d.path().join(FILE), "{").unwrap();
        assert!(!peek_run_as_admin(d.path()));
        assert!(!d.path().join("config.json.tmp").exists() && std::fs::read_dir(d.path()).unwrap().count() == 1);
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let d = tempfile::tempdir().unwrap();
        save(d.path(), &cfg()).unwrap();
        assert!(!d.path().join("config.json.tmp").exists());
    }
}
