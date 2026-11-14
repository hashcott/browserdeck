import type { Button, Theme } from "./types";

const HEX = /^#[0-9a-f]{6}$/i;
// Fallbacks (the keycap theme) for any value that is not a plain colour or an in-range number.
const hex = (v: unknown, d: string): string => (typeof v === "string" && HEX.test(v) ? v : d);
const num = (v: unknown, max: number, d: number): number =>
  typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= max ? v : d;

/** Theme values reach the DOM only as CSS custom properties, never as stylesheet or HTML text.
 *  Every value is re-checked here, so a hostile theme object cannot inject CSS. */
export function themeVars(theme: Theme): Record<string, string> {
  const k = theme.key;
  const ground = hex(theme.ground, "#22252b");
  return {
    "--key-radius": `${num(k.radius, 50, 9)}%`,
    "--key-gap": `${num(k.gap, 20, 9)}%`,
    "--key-gap-n": String(num(k.gap, 20, 9)),
    "--key-border": `${num(k.border_width, 8, 1)}px solid ${hex(k.border_color, "#1a1c20")}`,
    "--ground": ground,
    "--ink": hex(theme.ink, "#f6f1e4"),
    // Selected-key outline: contrasts with the ground the keys sit on.
    "--key-ring": pickInk(ground, theme, 3),
    "--scheme": isLight(ground) ? "light" : "dark",
    "--c-launch": hex(theme.actions.launch, "#f6f1e4"),
    "--c-hotkey": hex(theme.actions.hotkey, "#2c3a55"),
    "--c-volume": hex(theme.actions.volume, "#6f7d4f"),
    "--c-obs_scene": hex(theme.actions.obs_scene, "#e8703a"),
    "--c-usage": hex(theme.actions.usage, "#4a7f86"),
    "--c-media": hex(theme.actions.media, "#a0413c"),
  };
}

export function keyColor(button: Button, theme: Theme): string {
  return button.color ?? theme.actions[button.action.type];
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1, 7), 16);
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}

/** Black or white, whichever has the higher WCAG contrast ratio against `background` ("#rrggbb"). */
export function keyInk(background: string): "#000000" | "#ffffff" {
  const l = luminance(background);
  // contrast with white = 1.05/(l+0.05); with black = (l+0.05)/0.05
  return (l + 0.05) / 0.05 > 1.05 / (l + 0.05) ? "#000000" : "#ffffff";
}

/** WCAG contrast ratio between two "#rrggbb" colours. */
export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** True when `background` reads as a light surface (black text would beat white on it). */
export const isLight = (background: string): boolean => keyInk(background) === "#000000";

/** The theme's own `ink` or `ground`, whichever contrasts more with `background`, when it reaches `min`;
 *  otherwise pure black or white. Theme colours are re-validated, so a hostile theme cannot reach the DOM. */
export function pickInk(background: string, theme: Theme, min: number): string {
  const best = [hex(theme.ink, "#f6f1e4"), hex(theme.ground, "#22252b")]
    .sort((p, q) => contrast(q, background) - contrast(p, background))[0];
  return contrast(best, background) >= min ? best.toLowerCase() : keyInk(background);
}
