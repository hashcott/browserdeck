import type { CSSProperties } from "react";
import "./key.css";
import { keyColor, keyInk, themeVars } from "./theme";
import type { Button, Theme } from "./types";

export interface KeyFaceProps {
  button: Button;
  theme: Theme;
  /** Resolved URL for an image icon (desktop and phone resolve it differently). */
  iconSrc?: string;
  pressed?: boolean;
  selected?: boolean;
  error?: boolean;
  disabled?: boolean;
}

/** One button face. Presentational only: the caller wraps it in a <button> and owns all behaviour. */
export function KeyFace({ button, theme, iconSrc, pressed, selected, error, disabled }: KeyFaceProps) {
  const raw = keyColor(button, theme);
  const bg = /^#[0-9a-f]{6}$/i.test(raw) ? raw : "#f6f1e4";
  const k = theme.key;
  const cls = [
    "key-face",
    `depth-${k.depth}`,
    `press-${k.press}`,
    `legend-${k.legend}`,
    pressed && "is-pressed",
    selected && "is-selected",
    error && "is-error",
    disabled && "is-disabled",
  ].filter(Boolean).join(" ");
  const style = { ...themeVars(theme), "--key-bg": bg, "--key-ink": keyInk(bg) } as CSSProperties;
  const icon = button.icon;
  return (
    <div className={cls} style={style}>
      <div className="key-cap">
      {icon.kind === "emoji" ? (
        icon.value && <span className="key-icon" aria-hidden={button.label ? true : undefined}>{icon.value}</span>
      ) : (
        iconSrc && <img className="key-icon" src={iconSrc} alt="" draggable={false} />
      )}
      {button.label && <span className="key-label">{button.label}</span>}
      </div>
    </div>
  );
}
