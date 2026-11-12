import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createT, resolveLang } from "../../shared/i18n";
import type { ActionType, Button, Config, ObsState, Runtime, Status, Theme } from "../../shared/types";
import * as api from "../api";
import { EditorView, type Confirm, type EditorHandlers, type Notice } from "./EditorView";
import type { TestResult } from "./Inspector";
import { defaultAction, moveButton, newButton, placeNew, removeButton, updateButton } from "./layout";
import { createSaveQueue } from "./saveQueue";

const SLOW_MS = 300;
const TOAST_MS = 6000;
const IMAGE_EXT = ["png", "jpg", "jpeg", "webp"];

export interface EditorProps {
  /** The "pair phone" button; Task 10 wires it to the pairing screen. */
  onPair(): void;
}

export function Editor({ onPair }: EditorProps) {
  const [config, setConfig] = useState<Config | null>(null);
  const [theme, setTheme] = useState<Theme | null>(null);
  const [runtime, setRuntime] = useState<Runtime | null>(null);
  const [status, setStatus] = useState<Status>({ phone_connected: false, obs: "disconnected" });
  const [scenes, setScenes] = useState<string[]>([]);
  const [pageId, setPageId] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<Confirm>(null);
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const [toast, setToast] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [testing, setTesting] = useState(false);
  const [focusReq, setFocusReq] = useState<{ slot: number; tick: number } | null>(null);
  const configRef = useRef<Config | null>(null);
  const slowTimer = useRef<ReturnType<typeof setTimeout>>();
  const toastTimer = useRef<ReturnType<typeof setTimeout>>();

  const lang = resolveLang(config?.settings.language ?? "auto", navigator.language);
  const t = useMemo(() => createT(lang), [lang]);
  useEffect(() => { document.documentElement.lang = lang; }, [lang]);
  const tRef = useRef(t);
  tRef.current = t;

  const showError = useCallback((code: unknown) => {
    const key = `error.${String(code)}`;
    const text = tRef.current(key) === key ? tRef.current("error.unknown") : tRef.current(key);
    setToast(text);
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), TOAST_MS);
  }, []);

  // Saves are serialised; a rejected one puts the last accepted config back. Created once the config is loaded.
  const saver = useRef<ReturnType<typeof createSaveQueue<Config>>>();

  const commit = useCallback((next: Config, slow = false) => {
    configRef.current = next;
    setConfig(next);
    clearTimeout(slowTimer.current);
    slowTimer.current = undefined;
    if (slow) slowTimer.current = setTimeout(() => { slowTimer.current = undefined; saver.current!.save(configRef.current!); }, SLOW_MS);
    else saver.current!.save(next);
  }, []);
  /** Sends a pending debounced edit now and waits until the core has it. */
  const flush = useCallback(async () => {
    if (slowTimer.current) { clearTimeout(slowTimer.current); slowTimer.current = undefined; saver.current?.save(configRef.current!); }
    await saver.current?.idle();
  }, []);

  // Load, then follow status events.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const [cfg, rt, st] = await Promise.all([api.getConfig(), api.getRuntime(), api.getStatus()]);
        if (!alive) return;
        saver.current = createSaveQueue<Config>(api.saveConfig, () => {}, (err, good) => {
          if (good) { configRef.current = good; setConfig(good); }
          showError(err);
        }, cfg);
        configRef.current = cfg;
        setConfig(cfg); setRuntime(rt); setStatus(st); setPageId(cfg.pages[0].id);
      } catch (e) { showError(e); }
      const off = await api.onCoreEvent((e) => {
        if (e.type === "phone_status") setStatus((s) => ({ ...s, phone_connected: e.connected }));
        else if (e.type === "obs_status") setStatus((s) => ({ ...s, obs: e.state }));
        else showError(e.error);
      });
      if (alive) unlisten = off; else off();
    })();
    return () => { alive = false; unlisten?.(); };
  }, [showError]);
  useEffect(() => () => { void flush(); clearTimeout(toastTimer.current); }, [flush]);

  const themeId = config?.settings.theme;
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

  const page = config?.pages.find((pg) => pg.id === pageId) ?? config?.pages[0];
  const withPage = useCallback((fn: (pg: NonNullable<typeof page>) => NonNullable<typeof page>, slow = false) => {
    const cur = configRef.current;
    const pg = cur?.pages.find((x) => x.id === pageId) ?? cur?.pages[0];
    if (!cur || !pg) return;
    commit({ ...cur, pages: cur.pages.map((x) => (x.id === pg.id ? fn(x) : x)) }, slow);
  }, [commit, pageId]);
  const focusOn = (slot: number) => setFocusReq((r) => ({ slot, tick: (r?.tick ?? 0) + 1 }));

  const on: EditorHandlers = {
    selectPage: (id) => { setPageId(id); setSelectedId(null); setConfirm(null); },
    addPage: () => {
      const cur = configRef.current!;
      const id = crypto.randomUUID();
      commit({ ...cur, pages: [...cur.pages, { id, name: t("phone.page", { n: String(cur.pages.length + 1) }), buttons: [] }] });
      on.selectPage(id);
    },
    renamePage: (id, name) => {
      const cur = configRef.current!;
      commit({ ...cur, pages: cur.pages.map((x) => (x.id === id ? { ...x, name } : x)) });
    },
    askDeletePage: (id) => setConfirm({ kind: "page", id }),
    deletePage: (id) => {
      const cur = configRef.current!;
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
      const b = newButton(crypto.randomUUID(), slot, defaultAction(type));
      withPage((pg) => placeNew(pg, slot, b));
      setSelectedId(b.id); setConfirm(null); focusOn(slot);
    },
    moveTo: (id, slot) => { withPage((pg) => moveButton(pg, id, slot)); setSelectedId(id); focusOn(slot); },
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
      const b = page?.buttons.find((x) => x.id === selectedId);
      if (!b) return;
      try {
        const path = await open({ multiple: false, filters: [{ name: t("editor.iconImage"), extensions: IMAGE_EXT }] });
        if (typeof path !== "string") return;
        const file = await api.importIcon(path);
        withPage((pg) => updateButton(pg, { ...b, icon: { kind: "image", file } }));
      } catch (e) { showError(e); }
    },
    pickFile: async () => {
      const b = page?.buttons.find((x) => x.id === selectedId);
      if (!b || b.action.type !== "launch") return;
      try {
        const path = await open({ multiple: false });
        if (typeof path !== "string") return;
        withPage((pg) => updateButton(pg, { ...b, action: { ...(b.action as Extract<Button["action"], { type: "launch" }>), path } }));
      } catch (e) { showError(e); }
    },
    dismissNotice: (id) => setDismissed((d) => new Set(d).add(id)),
    pair: onPair,
  };

  if (!config || !theme || !runtime || !page) return <p className="loading" role="status">{toast ?? t("common.loading")}</p>;

  const notices: Notice[] = [];
  if (runtime.recovered_backup) notices.push({ id: "backup", text: t("editor.recovered", { file: runtime.recovered_backup }) });
  if (runtime.server_error) {
    const key = `error.${runtime.server_error}`;
    notices.push({ id: "server", text: t(key) === key ? t("error.unknown") : t(key) });
  }
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
