import { useId, useState, type ReactNode } from "react";
import type { createT } from "../../shared/i18n";
import type { Button, ObsState, VolumeOp } from "../../shared/types";
import { formatArgs, parseArgs } from "./args";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
export interface TestResult { id: string; ok: boolean; text: string }
export interface InspectorProps {
  button: Button | null;
  /** The theme's colour for this button's action type (shown while `color` is null). */
  themeColor: string;
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

function Field({ label, children, id }: { label: string; children: ReactNode; id?: string }) {
  return <div className="fld"><label htmlFor={id}>{label}</label>{children}</div>;
}

export function Inspector(p: InspectorProps) {
  const { button: b, t } = p;
  const id = useId();
  if (!b) {
    return (
      <aside className="right" aria-labelledby={`${id}h`}>
        <h2 id={`${id}h`}>{t("editor.properties")}</h2>
        <p className="note">{t("editor.noSelection")}</p>
      </aside>
    );
  }
  const set = (patch: Partial<Button>, slow = false) => p.onChange({ ...b, ...patch }, slow);
  const a = b.action;
  const result = p.testResult?.id === b.id ? p.testResult : null;

  return (
    <aside className="right" aria-labelledby={`${id}h`}>
      <h2 id={`${id}h`}>{b.label || t(`action.${a.type}`)}</h2>

      <Field label={t("editor.label")} id={`${id}l`}>
        <input id={`${id}l`} className="in" value={b.label} maxLength={40} onChange={(e) => set({ label: e.target.value }, true)} />
      </Field>

      <Field label={t("editor.icon")} id={`${id}i`}>
        <div className="icon-row">
          {b.icon.kind === "image" && p.iconSrc && <img className="icon-prev" src={p.iconSrc} alt="" />}
          <input id={`${id}i`} className="in emoji" value={b.icon.kind === "emoji" ? b.icon.value : ""} maxLength={16}
            placeholder={b.icon.kind === "image" ? t("editor.iconImage") : t("editor.iconEmoji")}
            onChange={(e) => set({ icon: { kind: "emoji", value: e.target.value } }, true)} />
          <button type="button" className="btn small" onClick={p.onPickImage}><Icon name="image" size={16} />{t("editor.iconPick")}</button>
        </div>
      </Field>

      <Field label={t("editor.color")}>
        <div className="color-row">
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
                <input type="radio" name={`${id}v`} checked={a.op === op} onChange={() => set({ action: { ...a, op } })} />
                <span>{t(`volume.${op}`)}</span>
              </label>
            ))}
          </div>
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
            <button type="button" className="btn danger" autoFocus onClick={p.onDelete}><Icon name="trash" size={18} />{t("common.delete")}</button>
            <button type="button" className="btn ghost" onClick={p.onCancelDelete}>{t("common.cancel")}</button>
          </div>
        </div>
      ) : (
        <div className="row">
          <button type="button" className="btn cta" disabled={p.testing} onClick={p.onTest}><Icon name="play" size={16} />{t("common.test")}</button>
          <button type="button" className="btn ghost danger-text" onClick={p.onAskDelete}><Icon name="trash" size={18} />{t("common.delete")}</button>
        </div>
      )}
      <div className="result" role="status" aria-live="polite">
        {result && <span className={result.ok ? "ok" : "bad"}><Icon name={result.ok ? "check" : "alert"} size={16} />{result.text}</span>}
      </div>
    </aside>
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
