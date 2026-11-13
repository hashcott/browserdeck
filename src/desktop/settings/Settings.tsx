import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { createT } from "../../shared/i18n";
import type { Theme, ThemeInfo } from "../../shared/types";
import * as api from "../api";
import { fitsGrid } from "../editor/layout";
import type { LoadedStore } from "../useConfigStore";
import { SettingsScreen } from "./SettingsView";
import { windowsFlow, type WindowsFlowState } from "./windowsFlow";

type T = ReturnType<typeof createT>;
const errorText = (t: T, code: unknown) => {
  const key = `error.${String(code)}`;
  return t(key) === key ? t("error.unknown") : t(key);
};

export function Settings({ store, t, onBack }: { store: LoadedStore; t: T; onBack(): void }) {
  const { config, commit, latest, saveError } = store;
  const [themes, setThemes] = useState<ThemeInfo[]>([]);
  const [previews, setPreviews] = useState<Record<string, Theme>>({});
  const [gridError, setGridError] = useState(false);
  const [themeError, setThemeError] = useState<string | null>(null);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [win, setWin] = useState<WindowsFlowState>({ confirmAdmin: false, busy: false, error: null });
  const alive = useRef(true);
  const baseline = useRef(saveError?.n ?? 0);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  const loadThemes = useCallback(async () => {
    try {
      const list = await api.listThemes();
      if (!alive.current) return;
      setThemes(list);
      setThemeError(null);
      const got = await Promise.all(list.map((th) => api.getTheme(th.id).then((full) => [th.id, full] as const, () => null)));
      if (alive.current) setPreviews(Object.fromEntries(got.filter((x) => x !== null)));
    } catch (e) { if (alive.current) setThemeError(errorText(t, e)); }
  }, [t]);
  useEffect(() => { void loadThemes(); }, [loadThemes]);

  const edit = (fn: (c: typeof config) => typeof config) => commit(fn(latest() ?? config));
  const windows = windowsFlow({
    api,
    reload: () => Promise.all([store.reload(), store.refreshRuntime()]),
    set: (patch) => { if (alive.current) setWin((w) => ({ ...w, ...patch })); },
    errorText: (code) => errorText(t, code),
  });
  const { runtime } = store;

  return (
    <SettingsScreen
      config={config} themes={themes} previews={previews} obs={store.status.obs} gridError={gridError}
      themeError={themeError} confirmDeleteId={confirmId} t={t}
      windows={runtime.platform !== "windows" ? null : {
        ...win, elevated: runtime.elevated,
        adminInactiveReason: runtime.admin_error && errorText(t, runtime.admin_error),
        autostartError: runtime.autostart_error && errorText(t, runtime.autostart_error),
      }}
      saveError={saveError && saveError.n > baseline.current ? errorText(t, saveError.code) : null}
      on={{
        back: onBack,
        pickTheme: (id) => { setThemeError(null); edit((c) => ({ ...c, settings: { ...c.settings, theme: id } })); },
        importTheme: async () => {
          setThemeError(null);
          try {
            const path = await open({ multiple: false, filters: [{ name: t("theme.fileType"), extensions: ["json"] }] });
            if (typeof path !== "string") return;
            await api.importTheme(path);
            await loadThemes();
          } catch (e) { if (alive.current) setThemeError(errorText(t, e)); }
        },
        askDeleteTheme: setConfirmId,
        cancelDeleteTheme: () => setConfirmId(null),
        deleteTheme: async (id) => {
          setConfirmId(null);
          setThemeError(null);
          try {
            await store.flush();
            await api.deleteTheme(id);
            // The core switches to the default theme itself when the active one is deleted: take its config as it is now.
            await store.reload();
            await loadThemes();
          } catch (e) { if (alive.current) setThemeError(errorText(t, e)); }
        },
        setLanguage: (language) => edit((c) => ({ ...c, settings: { ...c.settings, language } })),
        setGrid: (cols, rows) => {
          const cur = latest() ?? config;
          const grid = { cols, rows };
          if (!fitsGrid(cur.pages, grid)) { setGridError(true); return false; }
          setGridError(false);
          commit({ ...cur, settings: { ...cur.settings, grid } });
          return true;
        },
        clearGridError: () => setGridError(false),
        setObs: (patch) => edit((c) => ({ ...c, settings: { ...c.settings, obs: { ...c.settings.obs, ...patch } } })),
        setPort: (port) => edit((c) => ({ ...c, settings: { ...c.settings, port } })),
        askAdmin: windows.askAdmin,
        cancelAdmin: windows.cancelAdmin,
        setAdmin: (enabled) => { void windows.setAdmin(enabled); },
        setAutostart: (enabled) => { void windows.setAutostart(enabled); },
      }}
    />
  );
}
