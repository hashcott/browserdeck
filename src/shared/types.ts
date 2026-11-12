// Mirrors the Rust serde output (snake_case). The Rust code is the source of truth:
// config.rs, themes.rs, hub.rs, state.rs, actions/mod.rs, commands.rs.

export type Icon = { kind: "emoji"; value: string } | { kind: "image"; file: string };
export type VolumeOp = "up" | "down" | "mute";
export type Action =
  | { type: "launch"; path: string; args: string[] }
  | { type: "hotkey"; keys: string[] }
  | { type: "volume"; op: VolumeOp }
  | { type: "obs_scene"; scene: string };
export type ActionType = Action["type"];

export interface Button {
  id: string;
  slot: number;
  label: string;
  icon: Icon;
  /** "#rrggbb"; null = theme colour for the action type. */
  color: string | null;
  action: Action;
}
export interface Page {
  id: string;
  name: string;
  buttons: Button[];
}
export interface Grid {
  cols: number;
  rows: number;
}
export interface ObsSettings {
  host: string;
  port: number;
  password: string;
}
export interface Settings {
  token: string;
  port: number;
  obs: ObsSettings;
  grid: Grid;
  /** "auto" | "en" | "vi" */
  language: string;
  run_as_admin: boolean;
  autostart: boolean;
  tray_notice_shown: boolean;
  preferred_ip: string | null;
  /** Theme id. */
  theme: string;
}
export interface Config {
  schema_version: number;
  settings: Settings;
  pages: Page[];
}

export type Depth = "flat" | "dish" | "raised" | "sticker";
export type Press = "sink" | "dim" | "lift";
export type Legend = "center" | "top_left" | "bottom_left";
export interface ActionColors {
  launch: string;
  hotkey: string;
  volume: string;
  obs_scene: string;
}
export interface KeyStyle {
  /** % of the button side, 0..=50 */
  radius: number;
  /** % of the button side, 0..=20 */
  gap: number;
  /** px, 0..=8 */
  border_width: number;
  border_color: string;
  depth: Depth;
  press: Press;
  legend: Legend;
}
export interface Theme {
  schema: number;
  id: string;
  name: string;
  author: string;
  ground: string;
  ink: string;
  actions: ActionColors;
  key: KeyStyle;
}
export interface ThemeInfo {
  id: string;
  name: string;
  author: string;
  builtin: boolean;
}

// Error codes: one const list per Rust enum, the union types are derived from them.
export const ACTION_ERRORS = [
  "unknown_button", "file_not_found", "launch_failed", "invalid_hotkey", "blocked_by_elevated_window",
  "volume_failed", "obs_not_connected", "obs_scene_missing", "obs_request_failed", "unsupported_platform",
  "timeout",
] as const;
export const CONFIG_ERRORS = [
  "no_pages", "bad_grid", "slot_out_of_range", "duplicate_slot", "duplicate_id", "bad_color",
  "label_too_long", "bad_icon_file", "bad_theme", "bad_port", "save_failed",
] as const;
export const THEME_ERRORS = [
  "too_large", "bad_json", "bad_schema", "bad_id", "bad_name", "bad_color", "out_of_range",
  "builtin_id", "not_found", "io",
] as const;
export const ICON_ERRORS = ["too_large", "bad_type", "io"] as const;

export type ActionError = (typeof ACTION_ERRORS)[number];
export type ConfigError = (typeof CONFIG_ERRORS)[number];
export type ThemeError = (typeof THEME_ERRORS)[number];
export type IconError = (typeof ICON_ERRORS)[number];

/** Every error code the core can return, deduplicated. Each has an `error.<code>` message. */
export const ALL_ERROR_CODES: readonly string[] = [
  ...new Set<string>([...ACTION_ERRORS, ...CONFIG_ERRORS, ...THEME_ERRORS, ...ICON_ERRORS]),
];

// Phone WebSocket protocol (hub.rs)
export type ServerMsg =
  | { type: "layout"; pages: Page[]; grid: Grid; language: string; theme: Theme }
  | { type: "result"; press_id: number; ok: boolean; error: ActionError | null }
  | { type: "kicked" };
export type ClientMsg = { type: "press"; press_id: number; button_id: string };

// Desktop events and command results (state.rs, commands.rs)
export type ObsState = "connected" | "disconnected" | "auth_failed" | "unsupported";
export type CoreEvent =
  | { type: "phone_status"; connected: boolean }
  | { type: "obs_status"; state: ObsState }
  | { type: "action_failed"; button_id: string; error: ActionError };

export interface Runtime {
  port: number;
  icons_dir: string;
  elevated: boolean;
  first_run: boolean;
  recovered_backup: string | null;
  /** std::env::consts::OS */
  platform: string;
  /** Short code (e.g. "bind_failed") when the phone server could not bind any port. */
  server_error: string | null;
}
export interface PairingAddress {
  ip: string;
  /** http://<ip>:<port>/?token=<token> */
  url: string;
}
export interface PairingInfo {
  addresses: PairingAddress[];
  selected: string | null;
}
export interface Status {
  phone_connected: boolean;
  obs: ObsState;
}
