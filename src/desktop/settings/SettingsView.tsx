import { useId, useState } from "react";
import type { createT } from "../../shared/i18n";
import { KeyFace } from "../../shared/KeyFace";
import type { Button, Config, ObsState, Theme, ThemeInfo } from "../../shared/types";
import { Icon, StatusDot } from "../editor/Icons";
import { CloseButton, Screen } from "../Screen";
import "../screens.css";
import { CommitText, NumField } from "./NumField";

type T = ReturnType<typeof createT>;

export interface SettingsHandlers {
  back(): void;
  pickTheme(id: string): void;
  importTheme(): void;
  askDeleteTheme(id: string): void;
  deleteTheme(id: string): void;
  cancelDeleteTheme(): void;
  setLanguage(lang: string): void;
  /** false = refused (buttons would fall outside the grid). */
  setGrid(cols: number, rows: number): boolean;
  clearGridError(): void;
  setObs(patch: Partial<Config["settings"]["obs"]>): void;
  setPort(port: number): void;
  /** Opens the confirmation; administrator mode is only turned on from there. */
  askAdmin(): void;
  cancelAdmin(): void;
  setAdmin(enabled: boolean): void;
  setAutostart(enabled: boolean): void;
}
/** The Windows section's state. */
export interface WindowsSection {
  /** This process runs as administrator (it stays so until restarted, even with the mode turned off). */
  elevated: boolean;
  confirmAdmin: boolean;
  /** A switch's command is running. */
  busy: boolean;
  error: string | null;
  /** Why administrator mode is on but not active, when known. */
  adminInactiveReason: string | null;
  /** The last start-with-Windows registration failed. */
  autostartError: string | null;
}
/** The settings sections, one shown at a time. */
export type SettingsSection = "theme" | "language" | "grid" | "obs" | "server" | "windows";
export interface SettingsViewProps {
  /** The section to open on; the first one when absent. */
  startAt?: SettingsSection;
  config: Config;
  themes: ThemeInfo[];
  previews: Record<string, Theme>;
  obs: ObsState;
  gridError: boolean;
  themeError: string | null;
  saveError: string | null;
  confirmDeleteId: string | null;
  /** null hides the Windows section (other systems). */
  windows: WindowsSection | null;
  t: T;
  on: SettingsHandlers;
}

const SAMPLE: Button[] = [
  { id: "a", slot: 0, label: "", icon: { kind: "glyph", set: "ph", name: "rocket-launch" }, color: null, action: { type: "launch", path: "", args: [], admin: false } },
  { id: "b", slot: 1, label: "", icon: { kind: "glyph", set: "ph", name: "keyboard" }, color: null, action: { type: "hotkey", keys: [] } },
  { id: "c", slot: 2, label: "", icon: { kind: "glyph", set: "ph", name: "speaker-high" }, color: null, action: { type: "volume", op: "up" } },
  { id: "d", slot: 3, label: "", icon: { kind: "glyph", set: "ph", name: "broadcast" }, color: null, action: { type: "obs_scene", scene: "" } },
];
const LANGS = ["auto", "en", "vi"] as const;
const OBS_TEXT: Record<ObsState, [string, string | null]> = {
  connected: ["status.obsConnected", null],
  disconnected: ["status.obsDisconnected", "settings.obsHelpDisconnected"],
  auth_failed: ["status.obsAuthFailed", "settings.obsHelpAuth"],
  unsupported: ["status.obsUnsupported", "settings.obsHelpUnsupported"],
};

export function SettingsScreen(p: SettingsViewProps) {
  const { t, on, config } = p;
  const s = config.settings;
  const id = useId();
  const [showPw, setShowPw] = useState(false);
  const [obsText, obsHelp] = OBS_TEXT[p.obs];
  const w = p.windows;

  const [cur, setCur] = useState<SettingsSection>(p.startAt ?? "theme");
  const sections: [SettingsSection, string][] = [
    ["theme", t("settings.theme")], ["language", t("settings.language")], ["grid", t("settings.grid")], ["obs", t("settings.obs")], ["server", t("settings.server")],
    ...(w ? [["windows", t("settings.windows")] as [SettingsSection, string]] : []),
  ];

  return (
    <Screen className="settings-screen" onClose={on.back}>
      <nav className="set-nav" aria-labelledby={`${id}nav`}>
        <h1 className="cat" id={`${id}nav`}>{t("settings.title")}</h1>
        <ul>
          {sections.map(([k, label]) => (
            <li key={k}>
              <button type="button" aria-controls={`${id}${k}`} aria-current={k === cur ? "page" : undefined} onClick={() => setCur(k)}>{label}</button>
            </li>
          ))}
        </ul>
      </nav>
      <main className="set-main">
        <div className="set-inner">
          {p.saveError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{p.saveError}</p>}

          <section className="panel" id={`${id}theme`} aria-labelledby={`${id}themeh`} hidden={cur !== "theme"}>
            <div className="panel-head">
              <h2 id={`${id}themeh`}>{t("settings.theme")}</h2>
              <button type="button" className="btn small" onClick={on.importTheme}><Icon name="folder" size={16} />{t("theme.import")}</button>
            </div>
            <p className="note lede">{t("settings.defaultsNote")}</p>
            {p.themeError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{p.themeError}</p>}
            <ul className="themes" role="radiogroup" aria-labelledby={`${id}themeh`}>
              {p.themes.map((th) => {
                const preview = p.previews[th.id];
                const on_ = s.theme === th.id;
                return (
                  <li key={th.id} className={on_ ? "theme-card on" : "theme-card"}>
                    <label className="theme-pick">
                      <input type="radio" name={`${id}theme`} checked={on_} onChange={() => on.pickTheme(th.id)} />
                      <span className="theme-prev" aria-hidden="true" style={preview ? { background: preview.ground } : undefined}>
                        {preview && SAMPLE.slice(0, 3).map((b) => <span key={b.id} className="theme-key"><KeyFace button={b} theme={preview} /></span>)}
                      </span>
                      {/* name and author are untrusted free text: plain React text, truncated by CSS */}
                      <span className="theme-meta">
                        <b className="clip" title={th.name}>{th.name}</b>
                        {th.author && <i className="clip" title={th.author}>{t("settings.themeBy", { author: th.author })}</i>}
                        {on_ && <span className="theme-now"><Icon name="check" size={14} />{t("settings.themeCurrent")}</span>}
                      </span>
                    </label>
                    {!th.builtin && (p.confirmDeleteId === th.id ? (
                      <div className="theme-confirm" role="group" aria-label={t("theme.delete")}>
                        <p className="clip2">{t("theme.deleteConfirm", { name: th.name })}</p>
                        <div className="row">
                          <button type="button" className="btn small danger" onClick={() => on.deleteTheme(th.id)}>{t("common.delete")}</button>
                          <button type="button" className="btn small" autoFocus onClick={on.cancelDeleteTheme}>{t("common.cancel")}</button>
                        </div>
                      </div>
                    ) : (
                      <button type="button" className="icon-btn" aria-label={`${t("theme.delete")}: ${th.name}`} onClick={() => on.askDeleteTheme(th.id)}><Icon name="trash" size={18} /></button>
                    ))}
                  </li>
                );
              })}
            </ul>
          </section>

          <section className="panel" id={`${id}language`} aria-labelledby={`${id}languageh`} hidden={cur !== "language"}>
            <h2 id={`${id}languageh`}>{t("settings.language")}</h2>
            <div className="seg" role="radiogroup" aria-labelledby={`${id}languageh`}>
              {LANGS.map((l) => (
                <label key={l}>
                  <input type="radio" name={`${id}lang`} checked={s.language === l} onChange={() => on.setLanguage(l)} />
                  <span>{l === "auto" ? t("settings.languageAuto") : t(`language.${l}`)}</span>
                </label>
              ))}
            </div>
          </section>

          <section className="panel" id={`${id}grid`} aria-labelledby={`${id}gridh`} hidden={cur !== "grid"}>
            <h2 id={`${id}gridh`}>{t("settings.grid")}</h2>
            <p className="note lede">{t("settings.defaultsNote")}</p>
            <div className="fields">
              <NumField label={t("settings.cols")} value={s.grid.cols} min={1} max={8} t={t} onType onCommit={(n) => on.setGrid(n, s.grid.rows)} onBlur={on.clearGridError} />
              <NumField label={t("settings.rows")} value={s.grid.rows} min={1} max={8} t={t} onType onCommit={(n) => on.setGrid(s.grid.cols, n)} onBlur={on.clearGridError} />
            </div>
            {p.gridError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{t("settings.gridNoFit")}</p>}
          </section>

          <section className="panel" id={`${id}obs`} aria-labelledby={`${id}obsh`} hidden={cur !== "obs"}>
            <h2 id={`${id}obsh`}>{t("settings.obs")}</h2>
            <div className="fields">
              <CommitText label={t("settings.obsHost")} value={s.obs.host} allowEmpty={false} onCommit={(host) => on.setObs({ host })} />
              <NumField label={t("settings.obsPort")} value={s.obs.port} min={1} max={65535} t={t} onCommit={(n) => on.setObs({ port: n })} />
            </div>
            <CommitText label={t("settings.obsPassword")} value={s.obs.password} allowEmpty type={showPw ? "text" : "password"}
              onCommit={(password) => on.setObs({ password })}
              after={<button type="button" className="btn small" aria-pressed={showPw} onClick={() => setShowPw((v) => !v)}>{t(showPw ? "common.hide" : "common.show")}</button>} />
            <div className={`obs-state ${p.obs}`} role="status">
              <StatusDot on={p.obs === "connected"} />
              <span><b>{t("settings.obsState")}: {t(obsText)}</b>{obsHelp && <span className="note">{t(obsHelp)}</span>}</span>
            </div>
          </section>

          <section className="panel" id={`${id}server`} aria-labelledby={`${id}serverh`} hidden={cur !== "server"}>
            <h2 id={`${id}serverh`}>{t("settings.server")}</h2>
            <NumField label={t("settings.port")} value={s.port} min={1} max={65535} t={t} onCommit={on.setPort} />
            <p className="note">{t("settings.portNote")}</p>
          </section>

          {w && (
            <section className="panel" id={`${id}windows`} aria-labelledby={`${id}windowsh`} hidden={cur !== "windows"}>
              <h2 id={`${id}windowsh`}>{t("settings.windows")}</h2>
              {w.error && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{w.error}</p>}
              <div className="opts">
                <div>
                  <label className="switch">
                    <input type="checkbox" role="switch" checked={s.run_as_admin} disabled={w.busy || w.confirmAdmin}
                      onChange={(e) => (e.target.checked ? on.askAdmin() : on.setAdmin(false))} />
                    <span>{t("settings.runAsAdmin")}</span>
                  </label>
                  {w.elevated && !s.run_as_admin && <p className="note">{t("settings.adminOffAfterRestart")}</p>}
                  {!w.elevated && s.run_as_admin && (
                    <p className="note bad">{w.adminInactiveReason ? `${t("settings.adminInactive")} ${w.adminInactiveReason}` : t("settings.adminInactive")}</p>
                  )}
                  <p className="note">{t("settings.gamesLimit")}</p>
                  {w.confirmAdmin && (
                    <div className="confirm-box" role="group" aria-label={t("settings.runAsAdmin")}>
                      <p className="warn"><Icon name="alert" size={18} /><span>{t("settings.adminWarning")} {t("settings.adminRisk")}</span></p>
                      <div className="row">
                        <button type="button" className="btn danger" onClick={() => on.setAdmin(true)}>{t("settings.adminTurnOn")}</button>
                        <button type="button" className="btn" autoFocus onClick={on.cancelAdmin}>{t("common.cancel")}</button>
                      </div>
                    </div>
                  )}
                </div>
                <div>
                  <label className="switch">
                    <input type="checkbox" role="switch" checked={s.autostart} disabled={w.busy}
                      onChange={(e) => on.setAutostart(e.target.checked)} />
                    <span>{t("settings.autostart")}</span>
                  </label>
                  {w.autostartError && <p className="note bad">{w.autostartError}</p>}
                </div>
              </div>
            </section>
          )}
        </div>
      </main>
      <div className="set-close"><CloseButton label={t("settings.back")} onClose={on.back} /></div>
    </Screen>
  );
}
