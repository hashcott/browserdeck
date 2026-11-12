import type { Button, Theme } from "./types";

/** Theme values reach the DOM only as CSS custom properties, never as stylesheet or HTML text. */
export function themeVars(theme: Theme): Record<string, string> {
  const k = theme.key;
  return {
    "--key-radius": `${k.radius}%`,
    "--key-gap": `${k.gap}%`,
    "--key-border": `${k.border_width}px solid ${k.border_color}`,
    "--ground": theme.ground,
    "--ink": theme.ink,
    "--c-launch": theme.actions.launch,
    "--c-hotkey": theme.actions.hotkey,
    "--c-volume": theme.actions.volume,
    "--c-obs_scene": theme.actions.obs_scene,
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
