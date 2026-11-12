import { expect, test } from "vitest";
import { createT, resolveLang } from "./i18n";
import en from "./locales/en.json";
import vi from "./locales/vi.json";
import { ALL_ERROR_CODES } from "./types";
import actionsRs from "../../src-tauri/src/actions/mod.rs?raw";
import commandsRs from "../../src-tauri/src/commands.rs?raw";
import configRs from "../../src-tauri/src/config.rs?raw";
import themesRs from "../../src-tauri/src/themes.rs?raw";

test("resolveLang follows an explicit setting", () => { expect(resolveLang("vi", "en-US")).toBe("vi"); });
test("resolveLang auto uses the browser language", () => { expect(resolveLang("auto", "vi-VN")).toBe("vi"); });
test("resolveLang falls back to English", () => {
  expect(resolveLang("auto", "fr-FR")).toBe("en");
  expect(resolveLang("xx", "vi")).toBe("en");
});
test("en and vi have exactly the same keys", () => {
  expect(Object.keys(vi).sort()).toEqual(Object.keys(en).sort());
});
test("every error code has a message in both languages", () => {
  for (const c of ALL_ERROR_CODES) {
    expect(en).toHaveProperty(`error.${c}`);
    expect(vi).toHaveProperty(`error.${c}`);
  }
});
test("createT interpolates and falls back to the key", () => {
  expect(createT("en")("no.such.key")).toBe("no.such.key");
  expect(createT("vi")("theme.deleteConfirm", { name: "X" })).toContain("X");
  expect(createT("en")("theme.deleteConfirm")).toContain("{name}");
});

function variants(src: string, name: string): string[] {
  const body = new RegExp(`pub enum ${name} \\{([^}]*)\\}`).exec(src)?.[1];
  if (body === undefined) throw new Error(`enum ${name} not found`);
  return body
    .replace(/\/\/.*$/gm, "")
    .split(",")
    .map((v) => v.trim())
    .filter(Boolean)
    .map((v) => v.replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase());
}
test("ALL_ERROR_CODES covers every error variant in the Rust enums", () => {
  const rust = [
    ...variants(actionsRs, "ActionError"),
    ...variants(configRs, "ConfigError"),
    ...variants(themesRs, "ThemeError"),
    ...variants(commandsRs, "IconError"),
  ];
  expect(rust.length).toBeGreaterThan(20);
  expect(rust.filter((c) => !ALL_ERROR_CODES.includes(c))).toEqual([]);
});
test("createT ignores object prototype names", () => {
  expect(createT("en")("constructor")).toBe("constructor");
  expect(createT("vi")("toString")).toBe("toString");
});
