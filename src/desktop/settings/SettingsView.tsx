import { useId, useState } from "react";
import type { createT } from "../../shared/i18n";
import { KeyFace } from "../../shared/KeyFace";
import type { Button, Config, ObsState, Theme, ThemeInfo } from "../../shared/types";
import { Icon, StatusDot } from "../editor/Icons";
import { ScreenBar } from "../ScreenBar";
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
}
export interface SettingsViewProps {
  config: Config;
  themes: ThemeInfo[];
  previews: Record<string, Theme>;
  obs: ObsState;
  gridError: boolean;
  themeError: string | null;
  saveError: string | null;
  confirmDeleteId: string | null;
  t: T;
  on: SettingsHandlers;
}

const SAMPLE: Button[] = [
  { id: "a", slot: 0, label: "", icon: { kind: "emoji", value: "🚀" }, color: null, action: { type: "launch", path: "", args: [] } },
  { id: "b", slot: 1, label: "", icon: { kind: "emoji", value: "⌨️" }, color: null, action: { type: "hotkey", keys: [] } },
  { id: "c", slot: 2, label: "", icon: { kind: "emoji", value: "🔊" }, color: null, action: { type: "volume", op: "up" } },
  { id: "d", slot: 3, label: "", icon: { kind: "emoji", value: "🎬" }, color: null, action: { type: "obs_scene", scene: "" } },
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

  return (
    <div className="screen">
      <ScreenBar title={t("settings.title")} backLabel={t("settings.back")} onBack={on.back} t={t} />
      <main className="page">
        <div className="page-inner settings">
          {p.saveError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{p.saveError}</p>}

          <section className="panel" aria-labelledby={`${id}t`}>
            <div className="panel-head">
              <h2 id={`${id}t`}>{t("settings.theme")}</h2>
              <button type="button" className="btn small" onClick={on.importTheme}><Icon name="folder" size={16} />{t("theme.import")}</button>
            </div>
            {p.themeError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{p.themeError}</p>}
            <ul className="themes" role="radiogroup" aria-labelledby={`${id}t`}>
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
                        <i className="clip" title={th.author}>{t("settings.themeBy", { author: th.author })}</i>
                        {on_ && <span className="theme-now"><Icon name="check" size={14} />{t("settings.themeCurrent")}</span>}
                      </span>
                    </label>
                    {!th.builtin && (p.confirmDeleteId === th.id ? (
                      <div className="theme-confirm" role="group" aria-label={t("theme.delete")}>
                        <p className="clip2">{t("theme.deleteConfirm", { name: th.name })}</p>
                        <div className="row">
                          <button type="button" className="btn small danger" onClick={() => on.deleteTheme(th.id)} autoFocus>{t("common.delete")}</button>
                          <button type="button" className="btn small" onClick={on.cancelDeleteTheme}>{t("common.cancel")}</button>
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

          <section className="panel" aria-labelledby={`${id}l`}>
            <h2 id={`${id}l`}>{t("settings.language")}</h2>
            <div className="seg" role="radiogroup" aria-labelledby={`${id}l`}>
              {LANGS.map((l) => (
                <label key={l}>
                  <input type="radio" name={`${id}lang`} checked={s.language === l} onChange={() => on.setLanguage(l)} />
                  <span>{l === "auto" ? t("settings.languageAuto") : t(`language.${l}`)}</span>
                </label>
              ))}
            </div>
          </section>

          <section className="panel" aria-labelledby={`${id}g`}>
            <h2 id={`${id}g`}>{t("settings.grid")}</h2>
            <div className="two">
              <NumField label={t("settings.cols")} value={s.grid.cols} min={1} max={8} t={t} onType onCommit={(n) => on.setGrid(n, s.grid.rows)} onBlur={on.clearGridError} />
              <NumField label={t("settings.rows")} value={s.grid.rows} min={1} max={8} t={t} onType onCommit={(n) => on.setGrid(s.grid.cols, n)} onBlur={on.clearGridError} />
            </div>
            {p.gridError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{t("settings.gridNoFit")}</p>}
          </section>

          <section className="panel" aria-labelledby={`${id}o`}>
            <h2 id={`${id}o`}>{t("settings.obs")}</h2>
            <div className="two">
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

          <section className="panel" aria-labelledby={`${id}s`}>
            <h2 id={`${id}s`}>{t("settings.server")}</h2>
            <NumField label={t("settings.port")} value={s.port} min={1} max={65535} t={t} onCommit={on.setPort} />
            <p className="note">{t("settings.portNote")}</p>
          </section>

          {/* Task 11 adds the administrator-mode and start-with-Windows sections here, after the server port. */}
        </div>
      </main>
    </div>
  );
}
