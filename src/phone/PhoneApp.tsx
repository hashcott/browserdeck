import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createT, resolveLang } from "../shared/i18n";
import { createConnection, type ConnStatus } from "./connection";
import { PhoneView, type Layout } from "./PhoneView";

const ERROR_MS = 4000;

export function PhoneApp() {
  const [layout, setLayout] = useState<Layout | null>(null);
  const [status, setStatus] = useState<ConnStatus>("connecting");
  const [pressed, setPressed] = useState<ReadonlySet<string>>(new Set());
  const [errors, setErrors] = useState<Record<string, string>>({});
  const conn = useRef<ReturnType<typeof createConnection> | null>(null);
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  useEffect(() => {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    const c = createConnection({ url: `${proto}//${location.host}/ws`, onLayout: setLayout, onStatus: setStatus });
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

  const setHeld = (id: string, on: boolean) =>
    setPressed((s) => {
      if (s.has(id) === on) return s;
      const n = new Set(s);
      if (on) n.add(id); else n.delete(id);
      return n;
    });
  const onDown = useCallback((id: string) => { setHeld(id, true); navigator.vibrate?.(10); }, []);
  const onUp = useCallback((id: string) => setHeld(id, false), []);

  // Fired from click, so keyboard activation works too; a pointer tap yields exactly one click.
  const onPress = useCallback(async (id: string) => {
    const r = await conn.current?.press(id);
    if (!r || r.ok) return;
    clearTimeout(timers.current.get(id));
    setErrors((e) => {
      const { [id]: _old, ...rest } = e; // re-insert so this error is the newest
      return { ...rest, [id]: r.error };
    });
    timers.current.set(id, setTimeout(() => setErrors((e) => {
      const { [id]: _gone, ...rest } = e;
      return rest;
    }), ERROR_MS));
  }, []);

  return (
    <PhoneView
      layout={layout}
      status={status}
      pressed={pressed}
      errors={errors}
      t={t}
      onDown={onDown}
      onUp={onUp}
      onPress={onPress}
      onReload={() => location.reload()}
    />
  );
}
