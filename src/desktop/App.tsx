import { useEffect, useMemo, useState } from "react";
import { createT, resolveLang } from "../shared/i18n";
import { Editor } from "./editor/Editor";
import "./editor/editor.css";
import { Pairing } from "./pairing/PairingPage";
import { Settings } from "./settings/Settings";
import { useConfigStore, type LoadedStore } from "./useConfigStore";

type View = "editor" | "pairing" | "settings";

export function App() {
  const store = useConfigStore();
  const [chosen, setChosen] = useState<View | null>(null);
  const lang = resolveLang(store.config?.settings.language ?? "auto", navigator.language);
  const t = useMemo(() => createT(lang), [lang]);
  useEffect(() => { document.documentElement.lang = lang; }, [lang]);

  const { config, runtime } = store;
  if (store.loadFailed) {
    const key = `error.${String(store.loadFailed)}`;
    return (
      <div className="screen loading" role="alert">
        <p>{t(key) === key ? t("error.unknown") : t(key)}</p>
        <button type="button" className="btn cta" onClick={store.retry}>{t("common.retry")}</button>
      </div>
    );
  }
  if (!config || !runtime) return <p className="screen loading" role="status">{t("common.loading")}</p>;
  const loaded = store as LoadedStore;
  // First run opens the pairing screen; after that the editor.
  const view = chosen ?? (runtime.first_run ? "pairing" : "editor");
  const toEditor = () => setChosen("editor");

  // The editor stays mounted so it keeps its state while another view is open.
  return (
    <>
      <div hidden={view !== "editor"}>
        <Editor store={loaded} t={t} onPair={() => setChosen("pairing")} onSettings={() => setChosen("settings")} />
      </div>
      {view === "pairing" && <Pairing store={loaded} t={t} onBack={toEditor} />}
      {view === "settings" && <Settings store={loaded} t={t} onBack={toEditor} />}
    </>
  );
}
