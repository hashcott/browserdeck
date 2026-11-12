import { useRef, useState, type CSSProperties } from "react";
import { KeyFace } from "../shared/KeyFace";
import { themeVars } from "../shared/theme";
import type { Button, ServerMsg } from "../shared/types";
import type { ConnStatus } from "./connection";
import "./phone.css";

export type Layout = Extract<ServerMsg, { type: "layout" }>;
export interface PhoneViewProps {
  layout: Layout | null;
  status: ConnStatus;
  /** Buttons currently held down. */
  pressed: ReadonlySet<string>;
  /** Button id -> error code, oldest first; the newest one is spoken in the live region. */
  errors: Readonly<Record<string, string>>;
  t: (key: string, vars?: Record<string, string>) => string;
  onDown(id: string): void;
  onUp(id: string): void;
  onPress(id: string): void;
  onReload(): void;
}

const int = (n: number, d: number) => (Number.isInteger(n) && n > 0 && n <= 64 ? n : d);

export function PhoneView(p: PhoneViewProps) {
  const { layout, status, t } = p;
  const scroller = useRef<HTMLDivElement>(null);
  const [page, setPage] = useState(0);
  const vars = layout ? themeVars(layout.theme) : {};

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
  const cols = layout ? int(layout.grid.cols, 3) : 3;
  const rows = layout ? int(layout.grid.rows, 4) : 4;
  const pages = layout?.pages ?? [];
  const empty = layout !== null && pages.every((pg) => pg.buttons.length === 0);
  const codes = Object.values(p.errors);
  const errorCode = codes[codes.length - 1];
  const errorLine = errorCode ? (t(`error.${errorCode}`) === `error.${errorCode}` ? t("error.unknown") : t(`error.${errorCode}`)) : "";

  const goTo = (i: number) => {
    const el = scroller.current;
    if (!el) return;
    const calm = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    el.scrollTo({ left: i * el.clientWidth, behavior: calm ? "auto" : "smooth" });
  };

  const face = (b: Button) => {
    const name = b.label || t(`action.${b.action.type}`);
    const id = b.id;
    return (
      <button
        key={id}
        type="button"
        className="key-btn"
        aria-label={name}
        disabled={!open}
        style={{ gridColumn: (b.slot % cols) + 1, gridRow: Math.floor(b.slot / cols) + 1 }}
        onPointerDown={() => p.onDown(id)}
        onPointerUp={() => p.onUp(id)}
        onPointerCancel={() => p.onUp(id)}
        onPointerLeave={() => p.onUp(id)}
        onClick={() => p.onPress(id)}
      >
        <KeyFace
          button={b}
          theme={layout!.theme}
          iconSrc={b.icon.kind === "image" ? `/icons/${encodeURIComponent(b.icon.file)}` : undefined}
          pressed={p.pressed.has(id)}
          error={id in p.errors}
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
            {pages.map((pg) => (
              <section key={pg.id} className="slide">
                <div className="deck-grid" style={{ "--cols": cols, "--rows": rows } as CSSProperties}>
                  {pg.buttons.filter((b) => b.slot >= 0 && b.slot < cols * rows).map(face)}
                </div>
              </section>
            ))}
          </div>
        )}
      </main>
      {pages.length > 1 && (
        <nav className="dots" aria-label={t("phone.pages")}>
          {pages.map((pg, i) => (
            <button
              key={pg.id}
              type="button"
              className="dot"
              aria-label={pg.name || t("phone.page", { n: String(i + 1) })}
              aria-current={i === page ? "true" : undefined}
              onClick={() => goTo(i)}
            />
          ))}
        </nav>
      )}
      <div className="toast" role="status" aria-live="polite">{errorLine && <span>{errorLine}</span>}</div>
    </div>
  );
}
