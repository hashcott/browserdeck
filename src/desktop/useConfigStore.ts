import { useEffect, useMemo, useRef, useState } from "react";
import type { Config, Runtime, Status } from "../shared/types";
import * as api from "./api";
import { createConfigCore } from "./configCore";

/** An error the user should see. `n` makes repeats distinct. */
export interface StoreError { code: unknown; n: number }

/** Stands in when get_runtime fails, so a config that did load is still usable. */
const NO_RUNTIME: Runtime = { port: 0, icons_dir: "", elevated: false, first_run: false, recovered_backup: null, platform: "", server_error: null };

/** The one owner of the config: every view reads it here and writes it through `commit`, so all saves share one
 *  serialised queue and one rollback-on-rejection path (see configCore). Also owns the live phone/OBS status.
 *  Rejected saves (`saveError`) and failed button presses from the phone (`actionError`) are kept apart. */
export function useConfigStore() {
  const [config, setConfig] = useState<Config | null>(null);
  const [runtime, setRuntime] = useState<Runtime | null>(null);
  const [runtimeFailed, setRuntimeFailed] = useState(false);
  const [status, setStatus] = useState<Status>({ phone_connected: false, obs: "disconnected" });
  const [saveError, setSaveError] = useState<StoreError | null>(null);
  const [actionError, setActionError] = useState<StoreError | null>(null);
  const [loadFailed, setLoadFailed] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const errorN = useRef(0);

  const core = useMemo(() => createConfigCore(api.saveConfig, {
    onConfig: setConfig,
    onSaveError: (code) => setSaveError({ code, n: ++errorN.current }),
  }), []);

  // Subscribe first, then load: an event that lands while getStatus() is in flight wins over its answer.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    const seen = { phone: false, obs: false };
    (async () => {
      const off = await api.onCoreEvent((e) => {
        if (e.type === "phone_status") { seen.phone = true; setStatus((s) => ({ ...s, phone_connected: e.connected })); }
        else if (e.type === "obs_status") { seen.obs = true; setStatus((s) => ({ ...s, obs: e.state })); }
        else setActionError({ code: e.error, n: ++errorN.current });
      });
      if (alive) unlisten = off; else { off(); return; }
      try {
        const [cfg, rt, st] = await Promise.all([api.getConfig(), api.getRuntime().catch(() => null), api.getStatus()]);
        if (!alive) return;
        core.replace(cfg);
        setRuntime(rt ?? NO_RUNTIME); setRuntimeFailed(rt === null); setLoadFailed(null);
        setStatus((s) => ({ phone_connected: seen.phone ? s.phone_connected : st.phone_connected, obs: seen.obs ? s.obs : st.obs }));
      } catch (e) { if (alive) setLoadFailed(e); }
    })();
    return () => { alive = false; unlisten?.(); };
  }, [core, attempt]);

  // Closing or hiding the window must not lose a debounced edit.
  useEffect(() => {
    const send = () => { void core.flush(); };
    window.addEventListener("pagehide", send);
    window.addEventListener("beforeunload", send);
    return () => { window.removeEventListener("pagehide", send); window.removeEventListener("beforeunload", send); };
  }, [core]);
  useEffect(() => () => { void core.flush(); }, [core]);

  return {
    config, runtime, runtimeFailed, status, saveError, actionError, loadFailed, retry: () => setAttempt((n) => n + 1),
    commit: core.commit, flush: core.flush, latest: core.latest,
    /** Re-reads the config from the core, for when the core changed it itself. Flushes pending saves first. */
    async reload() { await core.flush(); core.replace(await api.getConfig()); },
  };
}
export type ConfigStore = ReturnType<typeof useConfigStore>;
/** What a view needs once the store has loaded. */
export type LoadedStore = ConfigStore & { config: Config; runtime: Runtime };
