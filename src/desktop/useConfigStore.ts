import { useCallback, useEffect, useRef, useState } from "react";
import type { Config, Runtime, Status } from "../shared/types";
import * as api from "./api";
import { createSaveQueue } from "./editor/saveQueue";

const SLOW_MS = 300;

/** An error the user should see (a rejected save or a failed action). `n` makes repeats distinct. */
export interface StoreError { code: unknown; n: number }

/** The one owner of the config: every view reads it here and writes it through `commit`, so all saves share one
 *  serialised queue and one rollback-on-rejection path. Also owns the live phone/OBS status. */
export function useConfigStore() {
  const [config, setConfig] = useState<Config | null>(null);
  const [runtime, setRuntime] = useState<Runtime | null>(null);
  const [status, setStatus] = useState<Status>({ phone_connected: false, obs: "disconnected" });
  const [error, setError] = useState<StoreError | null>(null);
  const [loadFailed, setLoadFailed] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const configRef = useRef<Config | null>(null);
  const slowTimer = useRef<ReturnType<typeof setTimeout>>();
  // Saves are serialised; a rejected one puts the last accepted config back. Created once the config is loaded.
  const saver = useRef<ReturnType<typeof createSaveQueue<Config>>>();
  const errorN = useRef(0);
  const report = useCallback((code: unknown) => setError({ code, n: ++errorN.current }), []);

  /** `slow` = typing or dragging: the save is debounced. */
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
  /** The config as it is right now, including edits React has not rendered yet. */
  const latest = useCallback(() => configRef.current, []);

  // Subscribe first, then load: an event that lands while getStatus() is in flight wins over its answer.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    const seen = { phone: false, obs: false };
    (async () => {
      const off = await api.onCoreEvent((e) => {
        if (e.type === "phone_status") { seen.phone = true; setStatus((s) => ({ ...s, phone_connected: e.connected })); }
        else if (e.type === "obs_status") { seen.obs = true; setStatus((s) => ({ ...s, obs: e.state })); }
        else report(e.error);
      });
      if (alive) unlisten = off; else { off(); return; }
      try {
        const [cfg, rt, st] = await Promise.all([api.getConfig(), api.getRuntime(), api.getStatus()]);
        if (!alive) return;
        saver.current = createSaveQueue<Config>(api.saveConfig, () => {}, (err, good) => {
          if (good) { configRef.current = good; setConfig(good); }
          report(err);
        }, cfg);
        configRef.current = cfg;
        setConfig(cfg); setRuntime(rt); setLoadFailed(null);
        setStatus((s) => ({ phone_connected: seen.phone ? s.phone_connected : st.phone_connected, obs: seen.obs ? s.obs : st.obs }));
      } catch (e) { if (alive) setLoadFailed(e); }
    })();
    return () => { alive = false; unlisten?.(); };
  }, [report, attempt]);

  // Closing or hiding the window must not lose a debounced edit.
  useEffect(() => {
    const send = () => { void flush(); };
    window.addEventListener("pagehide", send);
    window.addEventListener("beforeunload", send);
    return () => { window.removeEventListener("pagehide", send); window.removeEventListener("beforeunload", send); };
  }, [flush]);
  useEffect(() => () => { void flush(); }, [flush]);

  return { config, runtime, status, error, report, loadFailed, retry: () => setAttempt((n) => n + 1), commit, flush, latest };
}
export type ConfigStore = ReturnType<typeof useConfigStore>;
/** What a view needs once the store has loaded. */
export type LoadedStore = ConfigStore & { config: Config; runtime: Runtime };
