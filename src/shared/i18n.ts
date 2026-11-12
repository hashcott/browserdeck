import en from "./locales/en.json";
import vi from "./locales/vi.json";

export type Lang = "en" | "vi";
const tables: Record<Lang, Record<string, string>> = { en, vi };

/** "auto" follows the browser language; anything unknown falls back to English. */
export function resolveLang(setting: string, navigatorLanguage: string): Lang {
  const s = setting === "auto" ? navigatorLanguage : setting;
  return s.toLowerCase().startsWith("vi") && (setting === "auto" || setting === "vi") ? "vi" : "en";
}

/** Missing in the language: English; missing in both: the key. `{name}` is replaced from `vars`. */
export function createT(lang: Lang): (key: string, vars?: Record<string, string>) => string {
  const own = (l: Lang, k: string) => (Object.prototype.hasOwnProperty.call(tables[l], k) ? tables[l][k] : undefined);
  return (key, vars) =>
    (own(lang, key) ?? own("en", key) ?? key).replace(/\{(\w+)\}/g, (m, k: string) => vars?.[k] ?? m);
}
