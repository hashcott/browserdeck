import { useCallback, useEffect, useRef, useState } from "react";
import type { createT } from "../../shared/i18n";
import * as api from "../api";
import type { LoadedStore } from "../useConfigStore";
import { PairingScreen, type PairingState } from "./PairingView";

type T = ReturnType<typeof createT>;
const errorText = (t: T, code: unknown) => {
  const key = `error.${String(code)}`;
  return t(key) === key ? t("error.unknown") : t(key);
};

export function Pairing({ store, t, onBack }: { store: LoadedStore; t: T; onBack(): void }) {
  const { runtime, config, commit, latest, saveError } = store;
  const baseline = useRef(saveError?.n ?? 0);
  const [state, setState] = useState<PairingState>({ kind: "loading" });
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  const load = useCallback(async () => {
    if (runtime.server_error) { setState({ kind: "server_error", text: errorText(t, runtime.server_error) }); return; }
    setState({ kind: "loading" });
    try {
      const info = await api.pairingInfo();
      if (alive.current) setState({ kind: "info", info });
    } catch (e) { if (alive.current) setState({ kind: "load_error", text: `${t("pairing.loadError")} ${errorText(t, e)}` }); }
  }, [runtime.server_error, t]);
  useEffect(() => { void load(); }, [load]);

  return (
    <PairingScreen
      state={state} phoneConnected={store.status.phone_connected} confirmingRegenerate={confirming} error={error ?? (saveError && saveError.n > baseline.current ? errorText(t, saveError.code) : null)} t={t}
      onPickIp={async (ip) => {
        const prev = state.kind === "info" ? state.info.selected : null;
        setState((s) => (s.kind === "info" ? { kind: "info", info: { ...s.info, selected: ip } } : s));
        const cur = latest() ?? config;
        commit({ ...cur, settings: { ...cur.settings, preferred_ip: ip } });
        await store.flush();
        // A rejected save rolled the config back: put the select back too (the store's error says why).
        if (alive.current && latest()?.settings.preferred_ip !== ip) setState((s) => (s.kind === "info" ? { kind: "info", info: { ...s.info, selected: prev } } : s));
      }}
      onAskRegenerate={() => { setConfirming(true); setError(null); }}
      onCancelRegenerate={() => setConfirming(false)}
      onConfirmRegenerate={async () => {
        setConfirming(false);
        try {
          const info = await api.regenerateToken();
          if (alive.current) { setState({ kind: "info", info }); setError(null); }
        } catch (e) { if (alive.current) setError(errorText(t, e)); }
      }}
      onRetry={() => { setError(null); void load(); }}
      onBack={onBack}
    />
  );
}
