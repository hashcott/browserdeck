import { useEffect, useId, useRef, useState, type CSSProperties } from "react";
import type { createT } from "../../shared/i18n";
import { glyphUrl, loadGlyphs, searchGlyphs, type GlyphEntry } from "../../shared/glyphs";
import type { GlyphSet } from "../../shared/types";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
const SHOWN = 120;

/** A bundled icon drawn in the current text colour. */
export function Glyph({ set, name, className = "glyph" }: { set: string; name: string; className?: string }) {
  const src = glyphUrl(set, name);
  return src ? <span className={className} aria-hidden="true" style={{ maskImage: `url(${src})`, WebkitMaskImage: `url(${src})` } as CSSProperties} /> : null;
}

/** The "Icons" button and its popover: Phosphor icons and app logos, searched by name. */
export function GlyphPicker({ t, onPick }: { t: T; onPick(e: GlyphEntry): void }) {
  const [at, setAt] = useState<DOMRect | null>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const close = () => { setAt(null); btn.current?.focus(); };
  return (
    <>
      <button ref={btn} type="button" className="btn small" aria-haspopup="dialog" aria-expanded={at !== null}
        onClick={(e) => setAt(at ? null : e.currentTarget.getBoundingClientRect())}>
        <Icon name="grid" size={16} />{t("editor.iconLibrary")}
      </button>
      {at && (
        <>
          <div className="menu-backdrop" onPointerDown={close} />
          <div className="menu pop glyph-pop" role="dialog" aria-label={t("editor.iconLibrary")}
            style={{ top: Math.min(at.bottom + 6, window.innerHeight - 420), right: Math.max(8, window.innerWidth - at.right) }}
            onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); close(); } }}>
            <Browser t={t} onPick={(g) => { onPick(g); close(); }} />
          </div>
        </>
      )}
    </>
  );
}

function Browser({ t, onPick }: { t: T; onPick(e: GlyphEntry): void }) {
  const id = useId();
  const [all, setAll] = useState<GlyphEntry[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [set, setSet] = useState<GlyphSet>("ph");
  const [q, setQ] = useState("");
  useEffect(() => { loadGlyphs().then(setAll, () => setFailed(true)); }, []);
  const found = all ? searchGlyphs(all, set, q) : [];
  return (
    <>
      <div className="seg two" role="radiogroup" aria-label={t("editor.iconLibrary")}>
        {(["ph", "si"] as const).map((s) => (
          <label key={s}>
            <input type="radio" name={`${id}s`} checked={set === s} onChange={() => setSet(s)} />
            <span>{t(`editor.glyphSet.${s}`)}</span>
          </label>
        ))}
      </div>
      <input className="in glyph-q" type="search" autoFocus value={q} placeholder={t("editor.iconSearch")} aria-label={t("editor.iconSearch")}
        spellCheck={false} onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && found[0]) onPick(found[0]); }} />
      {failed ? (
        <p className="note bad">{t("editor.glyphLoadFailed")}</p>
      ) : !all ? (
        <p className="note">{t("common.loading")}</p>
      ) : found.length === 0 ? (
        <p className="note">{t("editor.glyphNone", { q: q.trim() })}</p>
      ) : (
        <>
          {/* titles are plain React text; names were checked by glyphUrl */}
          <ul className="glyph-grid">
            {found.slice(0, SHOWN).map((g) => (
              <li key={g.name}>
                <button type="button" title={g.title} aria-label={g.title} onClick={() => onPick(g)}><Glyph set={g.set} name={g.name} /></button>
              </li>
            ))}
          </ul>
          {found.length > SHOWN && <p className="note">{t("editor.glyphMore", { shown: String(SHOWN), total: String(found.length) })}</p>}
        </>
      )}
    </>
  );
}
