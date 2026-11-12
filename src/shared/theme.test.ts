import { expect, test } from "vitest";
import keycapJson from "../../themes/keycap.json";
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
