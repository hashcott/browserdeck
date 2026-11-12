import { useEffect, useId, useState } from "react";
import type { createT } from "../../shared/i18n";

/** A whole-number field. Keeps what is typed; only a value inside min..=max is passed on. */
export function NumField({ label, value, min, max, t, onCommit, onBlur }: {
  label: string; value: number; min: number; max: number; t: ReturnType<typeof createT>;
  onCommit(n: number): void; onBlur?(): void;
}) {
  const id = useId();
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const n = Number(draft);
  const valid = /^\d{1,5}$/.test(draft) && n >= min && n <= max;
  return (
    <div className="fld">
      <label htmlFor={id}>{label}</label>
      <input id={id} className="in num" inputMode="numeric" autoComplete="off" value={draft}
        aria-invalid={!valid} aria-describedby={valid ? undefined : `${id}h`}
        onChange={(e) => {
          setDraft(e.target.value);
          const v = Number(e.target.value);
          if (/^\d{1,5}$/.test(e.target.value) && v >= min && v <= max) onCommit(v);
        }}
        onBlur={() => { setDraft(String(value)); onBlur?.(); }} />
      {!valid && <p className="note bad" id={`${id}h`}>{t("settings.range", { min: String(min), max: String(max) })}</p>}
    </div>
  );
}
