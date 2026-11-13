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
const on = { back: noop, pickTheme: noop, importTheme: noop, askDeleteTheme: noop, deleteTheme: noop, cancelDeleteTheme: noop, setLanguage: noop, setGrid: () => true, clearGridError: noop, setObs: noop, setPort: noop,
  askAdmin: noop, cancelAdmin: noop, setAdmin: noop, setAutostart: noop };
const render = (extra = {}) => renderToStaticMarkup(
  <SettingsScreen config={config} themes={[{ id: "keycap", name: "Keycap", author: "Us", builtin: true }]} previews={{ keycap: keycap as Theme }}
    obs="connected" gridError={false} themeError={null} saveError={null} confirmDeleteId={null} windows={null} t={t} on={on} {...extra} />);
const win = { elevated: false, confirmAdmin: false, busy: false, error: null, adminInactiveReason: null, autostartError: null };
const adminOn = { ...config, settings: { ...config.settings, run_as_admin: true } };

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
test("the Windows section exists only on Windows", () => {
  expect(render()).not.toContain(t("settings.runAsAdmin"));
  expect(render()).not.toContain('role="switch"');
  const html = render({ windows: win });
  expect(html).toContain(`>${t("settings.windows")}</h2>`);
  expect(html.match(/<input type="checkbox" role="switch"/g)).toHaveLength(2);
  // each switch sits inside its own label with its text
  expect(html).toMatch(new RegExp(`<label class="switch"><input type="checkbox" role="switch"[^>]*/><span>${t("settings.runAsAdmin")}</span></label>`));
  expect(html).toMatch(new RegExp(`<label class="switch"><input type="checkbox" role="switch"[^>]*/><span>${t("settings.autostart")}</span></label>`));
});
test("the games limit is always under the administrator switch", () => {
  for (const windows of [win, { ...win, confirmAdmin: true }]) {
    const html = render({ windows });
    const admin = html.indexOf(t("settings.runAsAdmin"));
    expect(admin).toBeGreaterThan(-1);
    expect(html.indexOf(t("settings.gamesLimit"))).toBeGreaterThan(admin);
    expect(html.indexOf(t("settings.gamesLimit"))).toBeLessThan(html.indexOf(t("settings.autostart")));
  }
});
test("turning administrator mode on asks first, with both warnings, and the switch stays off meanwhile", () => {
  expect(render({ windows: win })).not.toContain(t("settings.adminWarning"));
  const html = render({ windows: { ...win, confirmAdmin: true } });
  expect(html).toContain(t("settings.adminWarning"));
  expect(html).toContain(t("settings.adminRisk"));
  expect(html).toContain(`>${t("settings.adminTurnOn")}</button>`);
  expect(html).toContain(`>${t("common.cancel")}</button>`);
  expect(html).not.toMatch(/role="switch" checked=""/);
});
test("turning administrator mode off says it takes effect after a restart", () => {
  expect(render({ windows: win })).not.toContain(t("settings.adminOffAfterRestart"));
  expect(render({ windows: { ...win, elevated: true } })).toContain(t("settings.adminOffAfterRestart"));
  const on_ = render({ windows: { ...win, elevated: true }, config: adminOn });
  expect(on_).not.toContain(t("settings.adminOffAfterRestart"));
  expect(on_).toMatch(/role="switch" checked=""/);
});
test("a failed Windows setting is shown in its section, and the switches wait while one runs", () => {
  const html = render({ windows: { ...win, busy: true, error: t("error.uac_declined") } });
  expect(html).toContain(t("error.uac_declined"));
  expect(html.match(/role="switch"[^>]*disabled=""/g)).toHaveLength(2);
});
test("administrator mode on but not elevated says so, and why when known", () => {
  const reason = t("error.uac_declined");
  const html = render({ windows: { ...win, adminInactiveReason: reason }, config: adminOn });
  expect(html).toContain(`${t("settings.adminInactive")} ${reason}`);
  expect(render({ windows: win, config: adminOn })).toContain(t("settings.adminInactive"));
  expect(render({ windows: { ...win, elevated: true }, config: adminOn })).not.toContain(t("settings.adminInactive"));
  expect(render({ windows: win })).not.toContain(t("settings.adminInactive"));
});
test("a start-with-Windows failure is shown under its switch", () => {
  expect(render({ windows: win })).not.toContain(t("error.autostart_failed"));
  const html = render({ windows: { ...win, autostartError: t("error.autostart_failed") } });
  expect(html.indexOf(t("error.autostart_failed"))).toBeGreaterThan(html.indexOf(t("settings.autostart")));
});
