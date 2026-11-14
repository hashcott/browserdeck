import { useRef, useState, type CSSProperties } from "react";
import { KeyFace, reading } from "../shared/KeyFace";
import { themeVars } from "../shared/theme";
import type { Button, ServerMsg, Theme, Usage } from "../shared/types";
import type { ConnStatus } from "./connection";
import "./phone.css";

export type Layout = Extract<ServerMsg, { type: "layout" }>;
export interface PhoneViewProps {
  layout: Layout | null;
  /** The latest usage sample, for usage keys. */
  usage?: Usage | null;
  status: ConnStatus;
  /** Buttons currently held down. */
  pressed: ReadonlySet<string>;
  /** [button id, error code] pairs, oldest first; the newest one is spoken in the live region. */
  errors: ReadonlyArray<readonly [string, string]>;
  t: (key: string, vars?: Record<string, string>) => string;
  onDown(id: string): void;
  onUp(id: string): void;
  onPress(id: string): void;
  onReload(): void;
}

const int = (n: unknown, d: number): number => (typeof n === "number" && Number.isInteger(n) && n > 0 && n <= 64 ? n : d);

export function PhoneView(p: PhoneViewProps) {
  const { layout, status, t } = p;
  const scroller = useRef<HTMLDivElement>(null);
  const [page, setPage] = useState(0);
  const pages = layout?.pages ?? [];
  const current = Math.min(page, Math.max(pages.length - 1, 0)); // pages may shrink on a layout update
  // The chrome (status strip, dots) follows the page in view; each slide carries its own theme.
  const vars = layout ? themeVars(pages[current]?.theme ?? layout.theme) : {};

  if (status === "kicked") {
    return (
      <div className="phone" style={vars as CSSProperties}>
        <main className="kicked">
          <p>{t("phone.kicked")}</p>
          <button type="button" className="reload" onClick={p.onReload}>{t("phone.reload")}</button>
        </main>
      </div>
    );
  }

  const open = status === "open";
  const empty = layout !== null && pages.every((pg) => pg.buttons.length === 0);
  const errorCode = p.errors[p.errors.length - 1]?.[1];
  const errorLine = errorCode ? (t(`error.${errorCode}`) === `error.${errorCode}` ? t("error.unknown") : t(`error.${errorCode}`)) : "";

  const goTo = (i: number) => {
    const el = scroller.current;
    if (!el) return;
    const calm = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    el.scrollTo({ left: i * el.clientWidth, behavior: calm ? "auto" : "smooth" });
  };

  const face = (b: Button, cols: number, theme: Theme) => {
    const r = reading(b, p.usage ?? null, t);
    const name = r ? `${b.label || r.name} ${r.value}` : b.label || t(`action.${b.action.type}`);
    const id = b.id;
    return (
      <button
        key={id}
        type="button"
        className="key-btn"
        aria-label={name}
        disabled={!open}
        style={{ "--x": (b.slot % cols) + 1, "--y": Math.floor(b.slot / cols) + 1 } as CSSProperties}
        onPointerDown={() => p.onDown(id)}
        onPointerUp={() => p.onUp(id)}
        onPointerCancel={() => p.onUp(id)}
        onPointerLeave={() => p.onUp(id)}
        onClick={() => p.onPress(id)}
      >
        <KeyFace
          button={b}
          theme={theme}
          iconSrc={b.icon.kind === "image" ? `/icons/${encodeURIComponent(b.icon.file)}` : undefined}
          reading={r}
          pressed={p.pressed.has(id)}
          error={p.errors.some(([e]) => e === id)}
          disabled={!open}
        />
      </button>
    );
  };

  return (
    <div className="phone" style={vars as CSSProperties}>
      {!open && <div className="strip" role="status">{t(status === "connecting" ? "phone.connecting" : "phone.disconnected")}</div>}
      <main className="stage">
        {empty ? (
          <p className="empty">{t("phone.empty")}</p>
        ) : (
          <div
            className="pager"
            ref={scroller}
            onScroll={(e) => setPage(Math.round(e.currentTarget.scrollLeft / (e.currentTarget.clientWidth || 1)))}
          >
            {pages.map((pg) => {
              const cols = int(pg.grid?.cols, 3);
              const keys = pg.buttons.filter((b) => b.slot >= 0 && b.slot < cols * int(pg.grid?.rows, 4));
              // Empty rows after the last key are left out, so the keys sit centred instead of above a blank band.
              const rows = Math.max(1, ...keys.map((b) => Math.floor(b.slot / cols) + 1));
              const theme = pg.theme ?? layout!.theme;
              return (
                <section key={pg.id} className="slide" style={themeVars(theme) as CSSProperties}>
                  <div className={`deck-grid${rows > cols ? " tall" : cols > rows ? " wide" : ""}`} style={{ "--cols": cols, "--rows": rows } as CSSProperties}>
                    {keys.map((b) => face(b, cols, theme))}
                  </div>
                </section>
              );
            })}
          </div>
        )}
        <div className="toast" role="status" aria-live="polite">{errorLine && <span>{errorLine}</span>}</div>
      </main>
      {pages.length > 1 && (
        <nav className="dots" aria-label={t("phone.pages")}>
          {pages.map((pg, i) => (
            <button
              key={pg.id}
              type="button"
              className="dot"
              aria-label={pg.name || t("phone.page", { n: String(i + 1) })}
              aria-current={i === current ? "true" : undefined}
              onClick={() => goTo(i)}
            />
          ))}
        </nav>
      )}
    </div>
  );
}
