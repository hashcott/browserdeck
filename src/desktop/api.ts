import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ActionError, Config, ConfigError, CoreEvent, IconError, PairingInfo, Runtime, Status, Theme,
  ThemeError, ThemeInfo,
} from "../shared/types";

// One function per command registered in src-tauri/src/lib.rs.
// A command that returns Result rejects with the error code string named in its doc comment
// (a snake_case code the UI translates with t(`error.${code}`)); the others have no error value.

export const getConfig = () => invoke<Config>("get_config");
/** Rejects with a ConfigError code. */
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });
export const getRuntime = () => invoke<Runtime>("get_runtime");
export const pairingInfo = () => invoke<PairingInfo>("pairing_info");
/** Rejects with a ConfigError code. */
export const regenerateToken = () => invoke<PairingInfo>("regenerate_token");
/** Rejects with an ActionError code. */
export const testButton = (buttonId: string) => invoke<void>("test_button", { buttonId });
/** Rejects with an ActionError code. */
export const obsScenes = () => invoke<string[]>("obs_scenes");
/** Copies the file at `path` into the icons folder; resolves with the stored file name. Rejects with an IconError code. */
export const importIcon = (path: string) => invoke<string>("import_icon", { path });
export const listThemes = () => invoke<ThemeInfo[]>("list_themes");
export const getTheme = (id: string) => invoke<Theme>("get_theme", { id });
/** Rejects with a ThemeError code. */
export const importTheme = (path: string) => invoke<ThemeInfo>("import_theme", { path });
/** Rejects with a ThemeError code. */
export const deleteTheme = (id: string) => invoke<void>("delete_theme", { id });
export const getStatus = () => invoke<Status>("get_status");

/** Subscribes to core events; resolves with the unsubscribe function. */
export const onCoreEvent = (cb: (e: CoreEvent) => void): Promise<() => void> =>
  listen<CoreEvent>("core-event", (e) => cb(e.payload));

export type { ActionError, ConfigError, IconError, ThemeError };
