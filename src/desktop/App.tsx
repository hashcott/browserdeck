import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createT, resolveLang } from "../shared/i18n";
import { AppFrame, type View } from "./AppFrame";
import { Editor } from "./editor/Editor";
import "./editor/editor.css";
import { Pairing } from "./pairing/PairingPage";
import { Rail } from "./Rail";
import { Settings } from "./settings/Settings";
import type { SettingsSection } from "./settings/SettingsView";
import { useConfigStore, type LoadedStore } from "./useConfigStore";

const TOAST_MS = 6000;

export function App() {
  const store = useConfigStore();
  const [chosen, setChosen] = useState<View | null>(null);
  const [settingsAt, setSettingsAt] = useState<SettingsSection>();
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout>>();
  const lang = resolveLang(store.config?.settings.language ?? "auto", navigator.language);
  const t = useMemo(() => createT(lang), [lang]);
  useEffect(() => { document.documentElement.lang = lang; }, [lang]);
  const tRef = useRef(t);
  tRef.current = t;

  const showToast = useCallback((text: string) => {
    setToast(text);
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), TOAST_MS);
  }, []);
  useEffect(() => () => clearTimeout(toastTimer.current), []);

  const { config, runtime, saveError, actionError } = store;
  const view: View = chosen ?? (runtime?.first_run ? "pairing" : "editor");
  const viewRef = useRef(view);
  viewRef.current = view;
  const errorText = (code: unknown) => {
    const key = `error.${String(code)}`;
    return tRef.current(key) === key ? tRef.current("error.unknown") : tRef.current(key);
  };
  // A failed button press from the phone is shown on every view. A rejected save is toasted here only for the
  // editor; Settings and Pairing say it inline next to what was being saved.
  useEffect(() => {
    if (actionError) showToast(tRef.current("editor.pressFailed", { reason: errorText(actionError.code) }));
  }, [actionError, showToast]);
  useEffect(() => {
    if (saveError && viewRef.current === "editor") showToast(errorText(saveError.code));
  }, [saveError, showToast]);

  if (store.loadFailed) {
    return (
      <div className="screen loading" role="alert">
        <p>{errorText(store.loadFailed)}</p>
        <button type="button" className="btn cta" onClick={store.retry}>{t("common.retry")}</button>
      </div>
    );
  }
  if (!config || !runtime) return <p className="screen loading" role="status">{t("common.loading")}</p>;
  const loaded = store as LoadedStore;
  const toEditor = () => setChosen("editor");
  const go = (v: View, section?: SettingsSection) => { setSettingsAt(section); setChosen(v); };

  return (
    <AppFrame view={view} toast={toast}
      rail={<Rail view={view} phoneConnected={loaded.status.phone_connected} t={t} onNavigate={(v) => go(v)} />}
      editor={<Editor store={loaded} t={t} showToast={showToast} onPair={() => go("pairing")} onObsSettings={() => go("settings", "obs")} />}>
      {view === "pairing" && <Pairing store={loaded} t={t} onBack={toEditor} />}
      {view === "settings" && <Settings store={loaded} t={t} startAt={settingsAt} onBack={toEditor} />}
    </AppFrame>
  );
}
