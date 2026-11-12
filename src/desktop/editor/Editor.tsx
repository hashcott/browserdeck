import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { createT } from "../../shared/i18n";
import type { ActionType, Button, ObsState, Theme } from "../../shared/types";
import * as api from "../api";
import { EditorView, type Confirm, type EditorHandlers, type Notice } from "./EditorView";
import type { TestResult } from "./Inspector";
import { defaultAction, moveButton, newButton, placeNew, removeButton, updateButton } from "./layout";
import type { LoadedStore } from "../useConfigStore";

const TOAST_MS = 6000;
const IMAGE_EXT = ["png", "jpg", "jpeg", "webp"];

export interface EditorProps {
  store: LoadedStore;
  t: ReturnType<typeof createT>;
  /** The "pair phone" button. */
  onPair(): void;
  /** The settings button in the top bar. */
  onSettings(): void;
}

export function Editor({ store, t, onPair, onSettings }: EditorProps) {
  const { config, runtime, status, commit, flush, latest } = store;
  const [theme, setTheme] = useState<Theme | null>(null);
  const [scenes, setScenes] = useState<string[]>([]);
  const [pageId, setPageId] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<Confirm>(null);
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const [toast, setToast] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [testing, setTesting] = useState(false);
  const [focusReq, setFocusReq] = useState<{ slot: number; tick: number } | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout>>();

  const tRef = useRef(t);
  tRef.current = t;

  const showToast = useCallback((text: string) => {
    setToast(text);
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), TOAST_MS);
  }, []);
  const showError = useCallback((code: unknown) => {
    const key = `error.${String(code)}`;
    showToast(tRef.current(key) === key ? tRef.current("error.unknown") : tRef.current(key));
  }, [showToast]);

  useEffect(() => () => clearTimeout(toastTimer.current), []);
  // A rejected save, or a button press from the phone that failed.
  const { saveError, actionError } = store;
  useEffect(() => { if (saveError) showError(saveError.code); }, [saveError, showError]);
  useEffect(() => {
    if (!actionError) return;
    const key = `error.${String(actionError.code)}`;
    const reason = tRef.current(key) === key ? tRef.current("error.unknown") : tRef.current(key);
    showToast(tRef.current("editor.pressFailed", { reason }));
  }, [actionError, showToast]);

  const themeId = config.settings.theme;
  useEffect(() => {
    if (!themeId) return;
    let alive = true;
    api.getTheme(themeId).then((th) => alive && setTheme(th), (e) => alive && showError(e));
    return () => { alive = false; };
  }, [themeId, showError]);

  // Scene list: refetched whenever the OBS state changes.
  const obs: ObsState = status.obs;
  useEffect(() => {
    if (obs !== "connected") { setScenes([]); return; }
    let alive = true;
    api.obsScenes().then((s) => alive && setScenes(s), () => alive && setScenes([]));
    return () => { alive = false; };
  }, [obs]);

  const page = config.pages.find((pg) => pg.id === pageId) ?? config.pages[0];
  const withPage = useCallback((fn: (pg: NonNullable<typeof page>) => NonNullable<typeof page>, slow = false) => {
    const cur = latest();
    const pg = cur?.pages.find((x) => x.id === pageId) ?? cur?.pages[0];
    if (!cur || !pg) return;
    commit({ ...cur, pages: cur.pages.map((x) => (x.id === pg.id ? fn(x) : x)) }, slow);
  }, [commit, pageId]);
  /** Applies `fn` to the button as it is now (not as it was when an async dialog opened); does nothing if it is gone. */
  const patchButton = (id: string, fn: (b: Button) => Button) => {
    const cur = latest();
    if (!cur?.pages.some((pg) => pg.buttons.some((b) => b.id === id))) return;
    commit({ ...cur, pages: cur.pages.map((pg) => (pg.buttons.some((b) => b.id === id) ? updateButton(pg, fn(pg.buttons.find((b) => b.id === id)!)) : pg)) });
  };
  const focusOn = (slot: number) => setFocusReq((r) => ({ slot, tick: (r?.tick ?? 0) + 1 }));

  const on: EditorHandlers = {
    selectPage: (id) => { setPageId(id); setSelectedId(null); setConfirm(null); },
    addPage: () => {
      const cur = latest()!;
      const id = crypto.randomUUID();
      commit({ ...cur, pages: [...cur.pages, { id, name: t("phone.page", { n: String(cur.pages.length + 1) }), buttons: [] }] });
      on.selectPage(id);
    },
    renamePage: (id, name) => {
      const cur = latest()!;
      commit({ ...cur, pages: cur.pages.map((x) => (x.id === id ? { ...x, name } : x)) });
    },
    askDeletePage: (id) => setConfirm({ kind: "page", id }),
    deletePage: (id) => {
      const cur = latest()!;
      if (cur.pages.length < 2) return;
      const i = cur.pages.findIndex((x) => x.id === id);
      const pages = cur.pages.filter((x) => x.id !== id);
      commit({ ...cur, pages });
      on.selectPage(pages[Math.min(i, pages.length - 1)].id);
    },
    cancelConfirm: () => {
      const slot = confirm?.kind === "button" ? page?.buttons.find((b) => b.id === confirm.id)?.slot : undefined;
      setConfirm(null);
      if (slot !== undefined) focusOn(slot);
    },
    select: (id) => { setSelectedId(id); setConfirm(null); },
    createAt: (slot, type: ActionType) => {
      const cur = latest()?.pages.find((x) => x.id === pageId) ?? latest()?.pages[0];
      if (cur?.buttons.some((x) => x.slot === slot)) { showToast(t("editor.slotTaken")); return; }
      const b = newButton(crypto.randomUUID(), slot, defaultAction(type));
      withPage((pg) => placeNew(pg, slot, b));
      setSelectedId(b.id); setConfirm(null); focusOn(slot);
    },
    moveTo: (id, slot) => {
      const cur = latest()?.pages.find((x) => x.id === pageId) ?? latest()?.pages[0];
      const b = cur?.buttons.find((x) => x.id === id);
      if (!b || b.slot === slot) return; // unchanged page: nothing to save or select
      withPage((pg) => moveButton(pg, id, slot));
      setSelectedId(id); focusOn(slot);
    },
    askDeleteButton: (id) => { setSelectedId(id); setConfirm({ kind: "button", id }); },
    deleteButton: (id) => {
      const slot = page?.buttons.find((b) => b.id === id)?.slot;
      withPage((pg) => removeButton(pg, id));
      setSelectedId(null); setConfirm(null);
      if (slot !== undefined) focusOn(slot);
    },
    change: (b: Button, slow) => withPage((pg) => updateButton(pg, b), slow),
    test: async (id) => {
      setTesting(true);
      setTestResult(null);
      try {
        await flush();
        await api.testButton(id);
        setTestResult({ id, ok: true, text: t("editor.testOk") });
      } catch (e) {
        const key = `error.${String(e)}`;
        setTestResult({ id, ok: false, text: t(key) === key ? t("error.unknown") : t(key) });
      } finally { setTesting(false); }
    },
    pickImage: async () => {
      const id = selectedId;
      if (!id) return;
      try {
        const path = await open({ multiple: false, filters: [{ name: t("editor.iconImage"), extensions: IMAGE_EXT }] });
        if (typeof path !== "string") return;
        const file = await api.importIcon(path);
        patchButton(id, (b) => ({ ...b, icon: { kind: "image", file } }));
      } catch (e) { showError(e); }
    },
    pickFile: async () => {
      const id = selectedId;
      if (!id) return;
      try {
        const path = await open({ multiple: false });
        if (typeof path !== "string") return;
        patchButton(id, (b) => (b.action.type === "launch" ? { ...b, action: { ...b.action, path } } : b));
      } catch (e) { showError(e); }
    },
    dismissNotice: (id) => setDismissed((d) => new Set(d).add(id)),
    pair: onPair,
    settings: onSettings,
  };

  if (!theme || !page) return <p className="loading" role="status">{toast ?? t("common.loading")}</p>;

  const notices: Notice[] = [];
  if (runtime.recovered_backup) notices.push({ id: "backup", text: t("editor.recovered", { file: runtime.recovered_backup }) });
  if (runtime.server_error) {
    const key = `error.${runtime.server_error}`;
    notices.push({ id: "server", text: t(key) === key ? t("error.unknown") : t(key) });
  }
  if (store.runtimeFailed) notices.push({ id: "runtime", text: t("editor.runtimeFailed") });
  const icons = runtime.icons_dir.replace(/[\\/]+$/, "");

  return (
    <EditorView
      config={config} theme={theme} pageId={page.id} selectedId={selectedId} confirm={confirm} status={status}
      scenes={scenes} platform={runtime.platform} notices={notices.filter((n) => !dismissed.has(n.id))} toast={toast}
      testResult={testResult} testing={testing} focusReq={focusReq} t={t}
      iconSrc={(b) => (b.icon.kind === "image" ? convertFileSrc(`${icons}/${b.icon.file}`) : undefined)}
      on={on}
    />
  );
}
