import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { createT } from "../../shared/i18n";
import type { Theme, ThemeInfo } from "../../shared/types";
import * as api from "../api";
import { fitsGrid } from "../editor/layout";
import type { LoadedStore } from "../useConfigStore";
import { SettingsScreen } from "./SettingsView";

type T = ReturnType<typeof createT>;
const errorText = (t: T, code: unknown) => {
  const key = `error.${String(code)}`;
  return t(key) === key ? t("error.unknown") : t(key);
};
const DEFAULT_THEME = "keycap";

export function Settings({ store, t, onBack }: { store: LoadedStore; t: T; onBack(): void }) {
  const { config, commit, latest } = store;
  const [themes, setThemes] = useState<ThemeInfo[]>([]);
  const [previews, setPreviews] = useState<Record<string, Theme>>({});
  const [gridError, setGridError] = useState(false);
  const [themeError, setThemeError] = useState<string | null>(null);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const alive = useRef(true);
  const baseline = useRef(store.error?.n ?? 0);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  const loadThemes = useCallback(async () => {
    try {
      const list = await api.listThemes();
      if (!alive.current) return;
      setThemes(list);
      const got = await Promise.all(list.map((th) => api.getTheme(th.id).then((full) => [th.id, full] as const, () => null)));
      if (alive.current) setPreviews(Object.fromEntries(got.filter((x) => x !== null)));
    } catch (e) { if (alive.current) setThemeError(errorText(t, e)); }
  }, [t]);
  useEffect(() => { void loadThemes(); }, [loadThemes]);

  const edit = (fn: (c: typeof config) => typeof config, slow = false) => commit(fn(latest() ?? config), slow);

  return (
    <SettingsScreen
      config={config} themes={themes} previews={previews} obs={store.status.obs} gridError={gridError}
      themeError={themeError} confirmDeleteId={confirmId} t={t}
      saveError={store.error && store.error.n > baseline.current ? errorText(t, store.error.code) : null}
      on={{
        back: onBack,
        pickTheme: (id) => edit((c) => ({ ...c, settings: { ...c.settings, theme: id } })),
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
            await api.deleteTheme(id);
            // The core falls back to the default theme when the active one is deleted; keep our copy in step.
            if (latest()?.settings.theme === id) edit((c) => ({ ...c, settings: { ...c.settings, theme: DEFAULT_THEME } }));
            await loadThemes();
          } catch (e) { if (alive.current) setThemeError(errorText(t, e)); }
        },
        setLanguage: (language) => edit((c) => ({ ...c, settings: { ...c.settings, language } })),
        setGrid: (cols, rows) => {
          const cur = latest() ?? config;
          const grid = { cols, rows };
          if (!fitsGrid(cur.pages, grid)) { setGridError(true); return; }
          setGridError(false);
          commit({ ...cur, settings: { ...cur.settings, grid } });
        },
        clearGridError: () => setGridError(false),
        setObs: (patch, slow) => edit((c) => ({ ...c, settings: { ...c.settings, obs: { ...c.settings.obs, ...patch } } }), slow),
        setPort: (port) => edit((c) => ({ ...c, settings: { ...c.settings, port } }), true),
      }}
    />
  );
}
