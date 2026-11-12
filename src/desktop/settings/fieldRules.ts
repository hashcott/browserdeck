/** The decisions behind the settings fields, kept pure so they can be tested without a DOM. */

/** Typed text -> the number to commit, or null when it is empty, not a plain whole number, or out of range. Leading zeros are fine. */
export function numFieldInput(text: string, min: number, max: number): number | null {
  if (!/^\d{1,5}$/.test(text)) return null;
  const n = Number(text);
  return n >= min && n <= max ? n : null;
}

/** Leaving the field: what to commit (only a valid, changed value) and what text to show. */
export function numFieldBlur(text: string, saved: number, min: number, max: number): { commit: number | null; show: string } {
  const n = numFieldInput(text, min, max);
  const commit = n !== null && n !== saved ? n : null;
  return { commit, show: String(n ?? saved) };
}

/** Leaving a text field: commit only a changed value (and a non-empty one unless `allowEmpty`); otherwise show the saved one. */
export function textFieldBlur(text: string, saved: string, allowEmpty: boolean): { commit: string | null; show: string } {
  const v = allowEmpty ? text : text.trim();
  if (v === saved || (!allowEmpty && v === "")) return { commit: null, show: saved };
  return { commit: v, show: v };
}
