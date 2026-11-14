//! Button themes: validated JSON data files (colours, bounded numbers and
//! keywords only). Validation is a security boundary: a theme can never carry
//! CSS, code, paths or URLs.
use crate::config::is_hex_color;
use serde::{Deserialize, Serialize};
use std::io::{self, Read};
use std::path::Path;

pub const THEME_SCHEMA: u32 = 1;
pub const DEFAULT_THEME_ID: &str = "keycap";
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub author: String,
    pub ground: String,
    pub ink: String,
    pub actions: ActionColors,
    pub key: KeyStyle,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActionColors {
    pub launch: String,
    pub hotkey: String,
    pub volume: String,
    pub obs_scene: String,
    /// Absent from themes made before usage keys existed.
    #[serde(default = "default_usage")]
    pub usage: String,
    /// Absent from themes made before media keys existed.
    #[serde(default = "default_media")]
    pub media: String,
}

fn default_usage() -> String {
    "#4a7f86".into()
}

fn default_media() -> String {
    "#a0413c".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KeyStyle {
    /// % of the button side, 0..=50.
    pub radius: u8,
    /// % of the button side, 0..=20.
    pub gap: u8,
    /// px, 0..=8.
    pub border_width: u8,
    pub border_color: String,
    pub depth: Depth,
    pub press: Press,
    pub legend: Legend,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Depth {
    Flat,
    Dish,
    Raised,
    Sticker,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Press {
    Sink,
    Dim,
    Lift,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Legend {
    Center,
    TopLeft,
    BottomLeft,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeError {
    TooLarge,
    BadJson,
    BadSchema,
    BadId,
    BadName,
    BadColor,
    OutOfRange,
    BuiltinId,
    NotFound,
    Io,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub builtin: bool,
}

fn is_valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn validate(t: &Theme) -> Result<(), ThemeError> {
    use ThemeError::*;
    if t.schema != THEME_SCHEMA {
        return Err(BadSchema);
    }
    if !is_valid_id(&t.id) {
        return Err(BadId);
    }
    if !(1..=40).contains(&t.name.chars().count()) || t.name.trim().is_empty() || t.author.chars().count() > 40 {
        return Err(BadName);
    }
    let (a, k) = (&t.actions, &t.key);
    let colours = [&t.ground, &t.ink, &a.launch, &a.hotkey, &a.volume, &a.obs_scene, &a.usage, &a.media, &k.border_color];
    if !colours.into_iter().all(|c| is_hex_color(c)) {
        return Err(BadColor);
    }
    if k.radius > 50 || k.gap > 20 || k.border_width > 8 {
        return Err(OutOfRange);
    }
    Ok(())
}

pub fn parse(json: &str) -> Result<Theme, ThemeError> {
    if json.len() as u64 > MAX_BYTES {
        return Err(ThemeError::TooLarge);
    }
    let t: Theme = serde_json::from_str(json).map_err(|_| ThemeError::BadJson)?;
    validate(&t)?;
    Ok(t)
}

pub fn builtins() -> Vec<Theme> {
    [
        include_str!("../../themes/keycap.json"),
        include_str!("../../themes/sticker.json"),
        include_str!("../../themes/arcade.json"),
        include_str!("../../themes/flat.json"),
    ]
    .into_iter()
    .map(|j| parse(j).expect("built-in theme is valid"))
    .collect()
}

fn is_builtin(id: &str) -> bool {
    builtins().iter().any(|t| t.id == id)
}

fn themes_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("themes")
}

/// Reads at most MAX_BYTES + 1 bytes, so an oversized file is never loaded whole.
fn read_limited(path: &Path) -> Result<String, ThemeError> {
    let file = std::fs::File::open(path).map_err(|_| ThemeError::Io)?;
    let mut s = String::new();
    match file.take(MAX_BYTES + 1).read_to_string(&mut s) {
        Ok(_) => Ok(s),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => Err(ThemeError::BadJson),
        Err(_) => Err(ThemeError::Io),
    }
}

/// An imported theme file is only valid if its `id` matches its file name.
fn load_imported(data_dir: &Path, id: &str) -> Option<Theme> {
    if !is_valid_id(id) || is_builtin(id) {
        return None;
    }
    let t = parse(&read_limited(&themes_dir(data_dir).join(format!("{id}.json"))).ok()?).ok()?;
    (t.id == id).then_some(t)
}

/// Built-ins first, then imported themes by file name. Unreadable or invalid files are skipped.
pub fn list(data_dir: &Path) -> Vec<ThemeInfo> {
    let info = |t: Theme, builtin| ThemeInfo { id: t.id, name: t.name, author: t.author, builtin };
    let mut out: Vec<_> = builtins().into_iter().map(|t| info(t, true)).collect();
    let mut ids: Vec<String> = std::fs::read_dir(themes_dir(data_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").map(str::to_owned))
        .collect();
    ids.sort();
    out.extend(ids.iter().filter_map(|id| load_imported(data_dir, id)).map(|t| info(t, false)));
    out
}

/// Unknown id or damaged file falls back to the default theme.
pub fn resolve(data_dir: &Path, id: &str) -> Theme {
    let all = builtins();
    let fallback = all.iter().find(|t| t.id == DEFAULT_THEME_ID).cloned().expect("default theme exists");
    all.into_iter().find(|t| t.id == id).or_else(|| load_imported(data_dir, id)).unwrap_or(fallback)
}

pub fn import(data_dir: &Path, source: &Path) -> Result<ThemeInfo, ThemeError> {
    let len = std::fs::metadata(source).map_err(|_| ThemeError::Io)?.len();
    if len > MAX_BYTES {
        return Err(ThemeError::TooLarge);
    }
    let t = parse(&read_limited(source)?)?;
    if is_builtin(&t.id) {
        return Err(ThemeError::BuiltinId);
    }
    let dir = themes_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|_| ThemeError::Io)?;
    let text = serde_json::to_string_pretty(&t).map_err(|_| ThemeError::Io)?;
    let tmp = dir.join(format!("{}.json.tmp", t.id));
    std::fs::write(&tmp, text).map_err(|_| ThemeError::Io)?;
    std::fs::rename(&tmp, dir.join(format!("{}.json", t.id))).map_err(|_| ThemeError::Io)?;
    Ok(ThemeInfo { id: t.id, name: t.name, author: t.author, builtin: false })
}

pub fn delete(data_dir: &Path, id: &str) -> Result<(), ThemeError> {
    if !is_valid_id(id) {
        return Err(ThemeError::BadId);
    }
    if is_builtin(id) {
        return Err(ThemeError::BuiltinId);
    }
    std::fs::remove_file(themes_dir(data_dir).join(format!("{id}.json"))).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ThemeError::NotFound,
        _ => ThemeError::Io,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keycap() -> Theme {
        builtins().into_iter().find(|t| t.id == "keycap").unwrap()
    }

    /// Serialise keycap, let `f` tamper with the raw JSON value, return the text.
    fn json_with(f: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut v = serde_json::to_value(keycap()).unwrap();
        f(&mut v);
        v.to_string()
    }

    #[test]
    fn all_builtins_parse_and_have_unique_ids() {
        let ids: Vec<_> = builtins().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["keycap", "sticker", "arcade", "flat"]);
    }

    #[test]
    fn builtin_files_round_trip() {
        for t in builtins() {
            assert_eq!(parse(&serde_json::to_string(&t).unwrap()), Ok(t));
        }
    }

    #[test]
    fn a_theme_from_before_usage_and_media_keys_gets_the_default_colours() {
        let j = json_with(|v| {
            let a = v["actions"].as_object_mut().unwrap();
            a.remove("usage");
            a.remove("media");
        });
        let t = parse(&j).unwrap();
        assert_eq!((t.actions.usage.as_str(), t.actions.media.as_str()), ("#4a7f86", "#a0413c"));
        let j = json_with(|v| v["actions"]["media"] = "red".into());
        assert_eq!(parse(&j), Err(ThemeError::BadColor));
        let j = json_with(|v| v["actions"]["usage"] = "red".into());
        assert_eq!(parse(&j), Err(ThemeError::BadColor));
    }

    #[test]
    fn rejects_unknown_field() {
        let j = json_with(|v| v["css"] = "body{}".into());
        assert_eq!(parse(&j), Err(ThemeError::BadJson));
        let j = json_with(|v| v["key"]["url"] = "x".into());
        assert_eq!(parse(&j), Err(ThemeError::BadJson));
    }

    #[test]
    fn rejects_css_smuggled_in_a_colour() {
        for bad in ["url(http://evil.example/x)", "#fff", "red", "#12345g", "#1234567", " #123456", "#123456;x"] {
            let j = json_with(|v| v["ground"] = bad.into());
            assert_eq!(parse(&j), Err(ThemeError::BadColor), "{bad}");
        }
        let j = json_with(|v| v["actions"]["obs_scene"] = "red".into());
        assert_eq!(parse(&j), Err(ThemeError::BadColor));
        let j = json_with(|v| v["key"]["border_color"] = "red".into());
        assert_eq!(parse(&j), Err(ThemeError::BadColor));
    }

    #[test]
    fn rejects_out_of_range_numbers() {
        for (k, n) in [("radius", 51), ("gap", 21), ("border_width", 9)] {
            let j = json_with(|v| v["key"][k] = n.into());
            assert_eq!(parse(&j), Err(ThemeError::OutOfRange), "{k}");
        }
        let j = json_with(|v| v["key"]["radius"] = 50.into());
        assert!(parse(&j).is_ok());
    }

    #[test]
    fn rejects_bad_id_and_name() {
        for id in ["../x", "A", "", "a b", &"a".repeat(41)] {
            let j = json_with(|v| v["id"] = id.into());
            assert_eq!(parse(&j), Err(ThemeError::BadId), "{id}");
        }
        for name in ["", "   ", &"x".repeat(41)] {
            let j = json_with(|v| v["name"] = name.into());
            assert_eq!(parse(&j), Err(ThemeError::BadName), "{name}");
        }
        let j = json_with(|v| v["name"] = "é".repeat(40).into());
        assert!(parse(&j).is_ok());
        let j = json_with(|v| v["author"] = "x".repeat(41).into());
        assert_eq!(parse(&j), Err(ThemeError::BadName));
    }

    #[test]
    fn rejects_wrong_schema() {
        let j = json_with(|v| v["schema"] = 2.into());
        assert_eq!(parse(&j), Err(ThemeError::BadSchema));
    }

    #[test]
    fn import_rejects_oversize_file() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("big.json");
        std::fs::write(&src, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert_eq!(import(d.path(), &src), Err(ThemeError::TooLarge));
        assert_eq!(parse(&" ".repeat(MAX_BYTES as usize + 1)), Err(ThemeError::TooLarge));
    }

    #[test]
    fn import_rejects_builtin_id() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("k.json");
        std::fs::write(&src, json_with(|_| {})).unwrap();
        assert_eq!(import(d.path(), &src), Err(ThemeError::BuiltinId));
    }

    #[test]
    fn import_then_list_and_resolve() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src.json");
        std::fs::write(&src, json_with(|v| v["id"] = "mine".into())).unwrap();
        let info = import(d.path(), &src).unwrap();
        assert_eq!(info, ThemeInfo { id: "mine".into(), name: "Keycap".into(), author: String::new(), builtin: false });
        let l = list(d.path());
        assert_eq!(l.len(), 5);
        assert!(l[..4].iter().all(|i| i.builtin));
        assert_eq!(l[4], info);
        assert_eq!(resolve(d.path(), "mine").id, "mine");
        // overwrite allowed
        import(d.path(), &src).unwrap();
        assert_eq!(list(d.path()).len(), 5);
        delete(d.path(), "mine").unwrap();
        assert_eq!(list(d.path()).len(), 4);
    }

    #[test]
    fn import_rewrites_normalised_json() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src.json");
        let j = json_with(|v| v["id"] = "mine".into());
        std::fs::write(&src, format!("\n\n   {j}   \n\n")).unwrap();
        import(d.path(), &src).unwrap();
        let saved = std::fs::read_to_string(d.path().join("themes/mine.json")).unwrap();
        assert_eq!(saved, serde_json::to_string_pretty(&parse(&j).unwrap()).unwrap());
    }

    #[test]
    fn import_missing_source_is_io() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(import(d.path(), &d.path().join("nope.json")), Err(ThemeError::Io));
    }

    #[test]
    fn resolve_falls_back_to_keycap() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(resolve(d.path(), "nope").id, "keycap");
        assert_eq!(resolve(d.path(), "../etc/passwd").id, "keycap");
        std::fs::create_dir(d.path().join("themes")).unwrap();
        std::fs::write(d.path().join("themes/broken.json"), "{nope").unwrap();
        assert_eq!(resolve(d.path(), "broken").id, "keycap");
        assert_eq!(list(d.path()).len(), 4);
        assert_eq!(resolve(d.path(), "flat").id, "flat");
    }

    #[test]
    fn delete_builtin_is_refused() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(delete(d.path(), "keycap"), Err(ThemeError::BuiltinId));
        assert_eq!(delete(d.path(), "nope"), Err(ThemeError::NotFound));
        assert_eq!(delete(d.path(), "../x"), Err(ThemeError::BadId));
    }
}
