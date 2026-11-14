import type { CSSProperties } from "react";
import "./key.css";
import { isLight, keyColor, pickInk, themeVars } from "./theme";
import type { Button, Theme, Usage } from "./types";

export interface KeyFaceProps {
  button: Button;
  theme: Theme;
  /** Resolved URL for an image icon (desktop and phone resolve it differently). */
  iconSrc?: string;
  pressed?: boolean;
  selected?: boolean;
  error?: boolean;
  disabled?: boolean;
  /** A usage key's live value, drawn in place of the icon, and the metric's name, the label while the button has none. */
  reading?: { value: string; name: string };
}

/** The reading for a usage key ("—" until the PC reports one); undefined for any other button. */
export function reading(b: Button, usage: Usage | null, t: (key: string) => string): KeyFaceProps["reading"] {
  if (b.action.type !== "usage") return undefined;
  const m = b.action.metric;
  const v = usage?.[m];
  return { value: typeof v === "number" ? `${v}${m === "temp" ? "°" : "%"}` : "—", name: t(`usage.${m}`) };
}

/** One button face. Presentational only: the caller wraps it in a <button> and owns all behaviour. */
export function KeyFace({ button, theme, iconSrc, pressed, selected, error, disabled, reading }: KeyFaceProps) {
  const raw = keyColor(button, theme);
  const bg = /^#[0-9a-f]{6}$/i.test(raw) ? raw : "#f6f1e4";
  const k = theme.key;
  const cls = [
    "key-face",
    `depth-${k.depth}`,
    `press-${k.press}`,
    `legend-${k.legend}`,
    isLight(bg) ? "key-light" : "key-dark",
    pressed && "is-pressed",
    selected && "is-selected",
    error && "is-error",
    disabled && "is-disabled",
  ].filter(Boolean).join(" ");
  const style = { ...themeVars(theme), "--key-bg": bg, "--key-ink": pickInk(bg, theme, 4.5) } as CSSProperties;
  const icon = button.icon;
  const label = button.label || reading?.name;
  return (
    <div className={cls} style={style}>
      <div className="key-cap">
      {reading ? (
        <span className="key-value">{reading.value}</span>
      ) : icon.kind === "emoji" ? (
        icon.value && <span className="key-icon" aria-hidden={button.label ? true : undefined}>{icon.value}</span>
      ) : (
        iconSrc && <img className="key-icon" src={iconSrc} alt="" draggable={false} />
      )}
      {label && <span className="key-label">{label}</span>}
      </div>
    </div>
  );
}
