import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createT, resolveLang } from "../shared/i18n";
import { createConnection, type ConnStatus } from "./connection";
import { PhoneView, type Layout } from "./PhoneView";
import { canFire, displayStatus, repeatable, visiblePressed } from "./pressState";
import type { Usage } from "../shared/types";

const ERROR_MS = 4000;

export function PhoneApp() {
  const [layout, setLayout] = useState<Layout | null>(null);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [rawStatus, setRawStatus] = useState<ConnStatus>("connecting");
  const [everOpen, setEverOpen] = useState(false);
  const [held, setHeld] = useState<ReadonlySet<string>>(new Set());
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());
  const [errors, setErrors] = useState<ReadonlyArray<readonly [string, string]>>([]);
  const layoutRef = useRef<Layout | null>(null);
  const pendingRef = useRef<ReadonlySet<string>>(new Set());
  const conn = useRef<ReturnType<typeof createConnection> | null>(null);
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  useEffect(() => {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    const c = createConnection({ url: `${proto}//${location.host}/ws`, onLayout: (m) => { layoutRef.current = m; setHeld(new Set()); setLayout(m); },
      onUsage: setUsage,
      onNewBuild: () => location.reload(),
      onStatus: (s) => {
        if (s === "open") setEverOpen(true);
        if (s !== "open") setHeld(new Set()); // a disabled key may never deliver pointerup
        setRawStatus(s);
      },
    });
    conn.current = c;
    const pending = timers.current;
    return () => {
      c.close();
      pending.forEach(clearTimeout);
    };
  }, []);

  const lang = resolveLang(layout?.language ?? "auto", navigator.language);
  const t = useMemo(() => createT(lang), [lang]);
  useEffect(() => { document.documentElement.lang = lang; }, [lang]);

  const status = displayStatus(rawStatus, everOpen);
  const toggle = (set: ReadonlySet<string>, id: string, on: boolean) => {
    if (set.has(id) === on) return set;
    const n = new Set(set);
    if (on) n.add(id); else n.delete(id);
    return n;
  };
  const onDown = useCallback((id: string) => { setHeld((s) => toggle(s, id, true)); navigator.vibrate?.(10); }, []);
  const onUp = useCallback((id: string) => setHeld((s) => toggle(s, id, false)), []);

  // Fired from click, so keyboard activation works too; a pointer tap yields exactly one click.
  const onPress = useCallback(async (id: string) => {
    const type = layoutRef.current?.pages.flatMap((pg) => pg.buttons).find((b) => b.id === id)?.action.type;
    if (!type || !canFire(type, id, pendingRef.current)) return;
    const guarded = !repeatable(type); // repeatable presses may overlap, so they are never marked pending
    if (guarded) { pendingRef.current = toggle(pendingRef.current, id, true); setPending(pendingRef.current); }
    const r = await conn.current?.press(id);
    if (guarded) { pendingRef.current = toggle(pendingRef.current, id, false); setPending(pendingRef.current); }
    if (!r || r.ok) return;
    clearTimeout(timers.current.get(id));
    setErrors((e) => [...e.filter(([k]) => k !== id), [id, r.error]]); // re-add so this error is the newest
    timers.current.set(id, setTimeout(() => setErrors((e) => e.filter(([k]) => k !== id)), ERROR_MS));
  }, []);

  return (
    <PhoneView
      layout={layout}
      usage={usage}
      status={status}
      pressed={visiblePressed(held, pending, layout, status)}
      errors={errors}
      t={t}
      onDown={onDown}
      onUp={onUp}
      onPress={onPress}
      onReload={() => location.reload()}
    />
  );
}
