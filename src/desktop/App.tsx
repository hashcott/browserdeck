import { useMemo, useState } from "react";
import { createT, resolveLang } from "../shared/i18n";
import { Editor } from "./editor/Editor";

/** Task 10 adds its two views ("pairing" and "settings") to this union and to the switch below. */
type View = "editor" | "pairing";

export function App() {
  const [view, setView] = useState<View>("editor");
  const t = useMemo(() => createT(resolveLang("auto", navigator.language)), []);
  // The editor stays mounted so it keeps its state while another view is open.
  return (
    <>
      <div hidden={view !== "editor"}><Editor onPair={() => setView("pairing")} /></div>
      {view === "pairing" && (
        <main style={{ padding: 24 }}>
          <h1>{t("pairing.title")}</h1>
          <button type="button" onClick={() => setView("editor")}>{t("common.close")}</button>
        </main>
      )}
    </>
  );
}
