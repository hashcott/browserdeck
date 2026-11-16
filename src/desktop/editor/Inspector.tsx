import { useId, useState, type CSSProperties, type ReactNode } from "react";
import type { createT } from "../../shared/i18n";
import type { Action, ActionType, Button, MediaOp, Metric, ObsState, VolumeOp } from "../../shared/types";
import { keyInk } from "../../shared/theme";
import { formatArgs, parseArgs } from "./args";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { defaultIcon, sameIcon } from "./layout";
import { Glyph, GlyphPicker } from "./GlyphPicker";
import { brandColor } from "../../shared/glyphs";
import { keyName } from "../../shared/KeyFace";
import { Icon } from "./Icons";
import "../screens.css"; // .switch

type T = ReturnType<typeof createT>;
export interface TestResult { id: string; ok: boolean; text: string }
export interface InspectorProps {
  button: Button | null;
  /** The theme's colour for this button's action type (shown while `color` is null). */
  themeColor: string;
  /** The active theme's four action colours, offered as presets. */
  presets: ReadonlyArray<{ type: ActionType; color: string }>;
  iconSrc?: string;
  platform: string;
  obs: ObsState;
  scenes: string[];
  confirming: boolean;
  testing: boolean;
  testResult: TestResult | null;
  t: T;
  /** `slow` = typing or dragging a colour: the editor debounces the save. */
  onChange(b: Button, slow: boolean): void;
  onPickImage(): void;
  onPickFile(): void;
  onTest(): void;
  onAskDelete(): void;
  onDelete(): void;
  onCancelDelete(): void;
}

const VOLUME: VolumeOp[] = ["up", "down", "mute"];
const METRICS: Metric[] = ["cpu", "ram", "temp"];
const MEDIA: MediaOp[] = ["play_pause", "previous", "next"];

function Field({ label, children, id }: { label: string; children: ReactNode; id?: string }) {
  return <div className="fld"><label htmlFor={id}>{label}</label>{children}</div>;
}

export function Inspector(p: InspectorProps) {
  const { button: b, t } = p;
  const id = useId();
  if (!b) {
    return (
      <section aria-labelledby={`${id}h`}>
        <h2 id={`${id}h`}>{t("editor.properties")}</h2>
        <p className="note">{t("editor.noSelection")}</p>
      </section>
    );
  }
  const set = (patch: Partial<Button>, slow = false) => p.onChange({ ...b, ...patch }, slow);
  // Choosing another operation: a key still showing the old default icon takes the new one; an icon the user set stays.
  const setAction = (action: Action) => set({ action, ...(sameIcon(b.icon, defaultIcon(b.action)) ? { icon: defaultIcon(action) } : {}) });
  const a = b.action;
  const result = p.testResult?.id === b.id ? p.testResult : null;

  return (
    <section aria-labelledby={`${id}h`}>
      <h2 id={`${id}h`}>{keyName(b, t)}</h2>

      <Field label={t("editor.label")} id={`${id}l`}>
        <input id={`${id}l`} className="in" value={b.label} maxLength={40} onChange={(e) => set({ label: e.target.value }, true)} />
      </Field>

      {a.type !== "usage" && <Field label={t("editor.icon")} id={`${id}i`}>
        <div className="icon-row">
          {b.icon.kind === "image" && p.iconSrc && <img className="icon-prev" src={p.iconSrc} alt="" />}
          {b.icon.kind === "glyph" && <span className="icon-prev"><Glyph set={b.icon.set} name={b.icon.name} /></span>}
          <input id={`${id}i`} className="in emoji" value={b.icon.kind === "emoji" ? b.icon.value : ""} maxLength={16}
            placeholder={b.icon.kind === "image" ? t("editor.iconImage") : b.icon.kind === "glyph" ? b.icon.name : t("editor.iconEmoji")}
            onChange={(e) => set({ icon: { kind: "emoji", value: e.target.value } }, true)} />
        </div>
        <div className="icon-row">
          {/* a logo brings its brand colour: NVIDIA green, Spotify green, YouTube red */}
          <GlyphPicker t={t} onPick={(g) => {
            const color = brandColor(g.hex);
            set({ icon: { kind: "glyph", set: g.set, name: g.name }, ...(color ? { color } : {}) });
          }} />
          <button type="button" className="btn small" onClick={p.onPickImage}><Icon name="image" size={16} />{t("editor.iconPick")}</button>
        </div>
      </Field>}

      <Field label={t("editor.color")}>
        <div className="color-row">
          {p.presets.map(({ type, color }) => (
            <button key={type} type="button" className="preset" style={{ "--c": color, "--on": keyInk(color) } as CSSProperties}
              aria-label={t("editor.presetColor", { name: t(`action.${type}`), hex: color })}
              aria-pressed={(b.color ?? p.themeColor).toLowerCase() === color.toLowerCase()}
              onClick={() => set({ color }, false)} />
          ))}
          <input type="color" className="swatch" aria-label={t("editor.color")} value={b.color ?? p.themeColor}
            onChange={(e) => set({ color: e.target.value }, true)} />
          <button type="button" className="btn small" aria-pressed={b.color === null} disabled={b.color === null}
            onClick={() => set({ color: null })}>{t("editor.themeColor")}</button>
        </div>
      </Field>

      {a.type === "launch" && (
        <>
          <Field label={t("editor.path")} id={`${id}p`}>
            <div className="icon-row">
              <input id={`${id}p`} className="in" value={a.path} spellCheck={false}
                onChange={(e) => set({ action: { ...a, path: e.target.value } }, true)} />
              <button type="button" className="btn small" onClick={p.onPickFile}><Icon name="folder" size={16} />{t("editor.chooseFile")}</button>
            </div>
          </Field>
          <Field label={t("editor.args")} id={`${id}a`}>
            <ArgsInput key={b.id} id={`${id}a`} args={a.args} onChange={(args) => set({ action: { ...a, args } }, true)} />
            <p className="note">{t("editor.argsHint")}</p>
          </Field>
          <div>
            <label className="switch">
              <input type="checkbox" role="switch" checked={a.admin}
                onChange={(e) => set({ action: { ...a, admin: e.target.checked } })} />
              <span>{t("editor.runAsAdmin")}</span>
            </label>
            <p className="note">{t("editor.runAsAdminHint")}</p>
          </div>
        </>
      )}

      {a.type === "hotkey" && (
        <Field label={t("action.hotkey")}>
          <HotkeyRecorder keys={a.keys} platform={p.platform} t={t} onChange={(keys) => set({ action: { ...a, keys } })} />
        </Field>
      )}

      {a.type === "volume" && (
        <Field label={t("action.volume")}>
          <div className="seg" role="radiogroup" aria-label={t("action.volume")}>
            {VOLUME.map((op) => (
              <label key={op}>
                <input type="radio" name={`${id}v`} checked={a.op === op} onChange={() => setAction({ ...a, op })} />
                <span>{t(`volume.${op}`)}</span>
              </label>
            ))}
          </div>
        </Field>
      )}

      {a.type === "media" && (
        <Field label={t("action.media")}>
          <div className="seg" role="radiogroup" aria-label={t("action.media")}>
            {MEDIA.map((op) => (
              <label key={op}>
                <input type="radio" name={`${id}d`} checked={a.op === op} onChange={() => setAction({ ...a, op })} />
                <span>{t(`media.${op}`)}</span>
              </label>
            ))}
          </div>
          <p className="note">{t("editor.mediaHint")}</p>
        </Field>
      )}

      {a.type === "usage" && (
        <Field label={t("action.usage")}>
          <div className="seg" role="radiogroup" aria-label={t("action.usage")}>
            {METRICS.map((metric) => (
              <label key={metric}>
                <input type="radio" name={`${id}m`} checked={a.metric === metric} onChange={() => set({ action: { ...a, metric } })} />
                <span>{t(`usage.${metric}`)}</span>
              </label>
            ))}
          </div>
          <p className="note">{t("editor.usageHint")}</p>
        </Field>
      )}

      {a.type === "obs_scene" && (
        <Field label={t("action.obs_scene")} id={`${id}s`}>
          {p.obs === "connected" ? (
            <select id={`${id}s`} className="in" value={a.scene} onChange={(e) => set({ action: { ...a, scene: e.target.value } })}>
              {a.scene === "" && <option value="" disabled>{t("editor.scenePick")}</option>}
              {a.scene !== "" && !p.scenes.includes(a.scene) && <option value={a.scene}>{t("editor.sceneMissing", { name: a.scene })}</option>}
              {p.scenes.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          ) : (
            <>
              <input id={`${id}s`} className="in" value={a.scene} onChange={(e) => set({ action: { ...a, scene: e.target.value } }, true)} />
              <p className="note">{t("editor.obsOffline")}</p>
            </>
          )}
        </Field>
      )}

      {p.confirming ? (
        <div className="confirm" role="group" aria-label={t("editor.deleteConfirm")}
          onKeyDown={(e) => e.key === "Escape" && p.onCancelDelete()}>
          <p>{t("editor.deleteConfirm")}</p>
          <div className="row">
            <button type="button" className="btn danger" onClick={p.onDelete}><Icon name="trash" size={18} />{t("common.delete")}</button>
            <button type="button" className="btn ghost" autoFocus onClick={p.onCancelDelete}>{t("common.cancel")}</button>
          </div>
        </div>
      ) : (
        <div className="row">
          {a.type !== "usage" && <button type="button" className="btn cta" disabled={p.testing} onClick={p.onTest}><Icon name="play" size={16} />{t("common.test")}</button>}
          <button type="button" className="btn ghost danger-text" onClick={p.onAskDelete}><Icon name="trash" size={18} />{t("common.delete")}</button>
        </div>
      )}
      <div className="result" role="status" aria-live="polite">
        {result && <span className={result.ok ? "ok" : "bad"}><Icon name={result.ok ? "check" : "alert"} size={16} />{result.text}</span>}
      </div>
    </section>
  );
}

/** Shows the formatted arguments (so a restored config is reflected); while focused it keeps the user's raw text. */
function ArgsInput({ id, args, onChange }: { id: string; args: string[]; onChange(a: string[]): void }) {
  const [raw, setRaw] = useState<string | null>(null);
  return (
    <input id={id} className="in" value={raw ?? formatArgs(args)} spellCheck={false}
      onFocus={() => setRaw(formatArgs(args))} onBlur={() => setRaw(null)}
      onChange={(e) => { setRaw(e.target.value); onChange(parseArgs(e.target.value)); }} />
  );
}
