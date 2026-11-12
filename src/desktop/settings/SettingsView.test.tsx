import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import keycap from "../../../themes/keycap.json";
import { createT } from "../../shared/i18n";
import type { Config, Theme } from "../../shared/types";
import { SettingsScreen } from "./SettingsView";

const t = createT("en");
const config = { schema_version: 1, pages: [], settings: { token: "", port: 47825, obs: { host: "localhost", port: 4455, password: "pw" },
  grid: { cols: 4, rows: 3 }, language: "auto", run_as_admin: false, autostart: false, tray_notice_shown: false, preferred_ip: null, theme: "keycap" } } as Config;
const noop = () => {};
const on = { back: noop, pickTheme: noop, importTheme: noop, askDeleteTheme: noop, deleteTheme: noop, cancelDeleteTheme: noop, setLanguage: noop, setGrid: noop, clearGridError: noop, setObs: noop, setPort: noop };
const render = (extra = {}) => renderToStaticMarkup(
  <SettingsScreen config={config} themes={[{ id: "keycap", name: "Keycap", author: "Us", builtin: true }]} previews={{ keycap: keycap as Theme }}
    obs="connected" gridError={false} themeError={null} saveError={null} confirmDeleteId={null} t={t} on={on} {...extra} />);

test("theme name and author are escaped text; built-ins have no delete control", () => {
  const html = render({ themes: [
    { id: "keycap", name: "Keycap", author: "Us", builtin: true },
    { id: "x", name: "<img src=x onerror=alert(1)>", author: "<b>me</b>", builtin: false },
  ] });
  expect(html).not.toContain("<img src=x");
  expect(html).toContain("&lt;img src=x onerror=alert(1)&gt;");
  expect(html.match(/icon-btn/g)).toHaveLength(1);
});
test("the password is hidden until shown", () => {
  expect(render()).toContain('type="password"');
});
test("every OBS state has its own message", () => {
  const texts = (["connected", "disconnected", "auth_failed", "unsupported"] as const).map((obs) => render({ obs }));
  expect(new Set(texts).size).toBe(4);
  expect(texts[3]).toContain("version 30.2 or newer");
});
test("grid message", () => {
  expect(render({ gridError: true })).toContain(t("settings.gridNoFit"));
});
