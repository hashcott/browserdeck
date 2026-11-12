import { useEffect, useId, useState } from "react";
import type { createT } from "../../shared/i18n";
import { numFieldBlur, numFieldInput, textFieldBlur } from "./fieldRules";

type T = ReturnType<typeof createT>;

/** A whole-number field. `onCommit` returns false to refuse the value (the field then shows the saved one again).
 *  `onType` = commit as typed (small ranges only); otherwise the value is committed once, on blur or Enter. */
export function NumField({ label, value, min, max, t, onCommit, onType, onBlur }: {
  label: string; value: number; min: number; max: number; t: T;
  onCommit(n: number): boolean | void; onType?: boolean; onBlur?(): void;
}) {
  const id = useId();
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const valid = numFieldInput(draft, min, max) !== null;
  const finish = () => {
    const r = numFieldBlur(draft, value, min, max);
    if (r.commit !== null && onCommit(r.commit) === false) setDraft(String(value)); else setDraft(r.show);
  };
  return (
    <div className="fld">
      <label htmlFor={id}>{label}</label>
      <input id={id} className="in num" inputMode="numeric" autoComplete="off" value={draft}
        aria-invalid={!valid} aria-describedby={valid ? undefined : `${id}h`}
        onChange={(e) => {
          setDraft(e.target.value);
          const n = numFieldInput(e.target.value, min, max);
          if (onType && n !== null && n !== value && onCommit(n) === false) setDraft(String(value));
        }}
        onKeyDown={(e) => { if (e.key === "Enter" && !onType) finish(); }}
        onBlur={() => { if (onType) setDraft(String(value)); else finish(); onBlur?.(); }} />
      {!valid && <p className="note bad" id={`${id}h`}>{t("settings.range", { min: String(min), max: String(max) })}</p>}
    </div>
  );
}

/** A text field committed once, on blur or Enter. An empty value is refused unless `allowEmpty`. */
export function CommitText({ label, value, allowEmpty, onCommit, type = "text", after }: {
  label: string; value: string; allowEmpty: boolean; onCommit(v: string): void; type?: "text" | "password"; after?: React.ReactNode;
}) {
  const id = useId();
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const finish = () => {
    const r = textFieldBlur(draft, value, allowEmpty);
    if (r.commit !== null) onCommit(r.commit);
    setDraft(r.show);
  };
  return (
    <div className="fld">
      <label htmlFor={id}>{label}</label>
      <div className={after ? "pw" : undefined}>
        <input id={id} className="in" type={type} value={draft} autoComplete="off" spellCheck={false}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") finish(); }} onBlur={finish} />
        {after}
      </div>
    </div>
  );
}
