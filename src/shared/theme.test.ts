import { expect, test } from "vitest";
import arcadeJson from "../../themes/arcade.json";
import flatJson from "../../themes/flat.json";
import keycapJson from "../../themes/keycap.json";
import stickerJson from "../../themes/sticker.json";
import { keyColor, keyInk, themeVars } from "./theme";
import type { Button, Theme } from "./types";

const theme = keycapJson as Theme;
const button = (over: Partial<Button>): Button => ({
  id: "b", slot: 0, label: "L", icon: { kind: "emoji", value: "x" }, color: null,
  action: { type: "volume", op: "up" }, ...over,
});

test("keyColor prefers the button's own colour", () => {
  expect(keyColor(button({ color: "#112233" }), theme)).toBe("#112233");
});
test("keyColor falls back to the theme colour for the action type", () => {
  expect(keyColor(button({}), theme)).toBe(theme.actions.volume);
  expect(keyColor(button({ action: { type: "obs_scene", scene: "s" } }), theme)).toBe(theme.actions.obs_scene);
});
test("keyInk picks white on dark and black on light", () => {
  expect(keyInk("#2c3a55")).toBe("#ffffff");
  expect(keyInk("#f6f1e4")).toBe("#000000");
});
test("themeVars emits only custom properties", () => {
  const v = themeVars(theme);
  expect(Object.keys(v).every((k) => k.startsWith("--"))).toBe(true);
  expect(v["--key-radius"]).toBe("9%");
  expect(v["--c-obs_scene"]).toBe("#e8703a");
});

function contrast(a: string, b: string): number {
  const lum = (h: string) => {
    const n = parseInt(h.slice(1), 16);
    const l = (c: number) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
    return 0.2126 * l((n >> 16) & 255) + 0.7152 * l((n >> 8) & 255) + 0.0722 * l(n & 255);
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
test("--key-ring contrasts at least 3:1 with every built-in theme's ground", () => {
  for (const t of [keycapJson, stickerJson, arcadeJson, flatJson] as Theme[]) {
    expect(contrast(themeVars(t)["--key-ring"], t.ground), t.id).toBeGreaterThanOrEqual(3);
  }
});
test("themeVars falls back to safe values for hostile input", () => {
  const evil = {
    ...theme, ground: "red; background:url(x)", ink: "url(x)",
    actions: { ...theme.actions, launch: "#fff} body{" },
    key: { ...theme.key, radius: Number.NaN, gap: 999, border_width: -1, border_color: "red;x:y" },
  } as Theme;
  const v = themeVars(evil);
  expect(v["--ground"]).toBe("#22252b");
  expect(v["--key-radius"]).toBe("9%");
  expect(v["--key-border"]).toBe("1px solid #1a1c20");
  for (const val of Object.values(v)) expect(val).toMatch(/^[#0-9a-fA-F% a-z]+$/);
  expect(Object.values(v).join("")).not.toMatch(/[;{}()]/);
});
