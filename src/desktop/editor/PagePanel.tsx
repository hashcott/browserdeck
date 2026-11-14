import { useEffect, useId, useRef, useState } from "react";
import type { createT } from "../../shared/i18n";
import type { Grid, Page, ThemeInfo } from "../../shared/types";
import { NumField } from "../settings/NumField";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
export interface PagePanelProps {
  page: Page;
  name: string;
  /** Settings' grid and theme, used while the page has none of its own. */
  defaultGrid: Grid;
  defaultTheme: string;
  themes: ThemeInfo[];
  t: T;
  /** false = refused: buttons would fall outside that grid. */
  onGrid(grid: Grid | null): boolean;
  onTheme(id: string | null): void;
  onOpenThemes(): void;
}

/** The toolbar's Page settings button and its popover, apart from the button properties panel. */
export function PageSettings(p: PagePanelProps) {
  const [at, setAt] = useState<DOMRect | null>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const pop = useRef<HTMLDivElement>(null);
  const close = () => { setAt(null); btn.current?.focus(); };
  useEffect(() => { if (at) pop.current?.querySelector<HTMLElement>("select, input")?.focus(); }, [at !== null]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => setAt(null), [p.page.id]);
  return (
    <>
      <button ref={btn} type="button" className="btn small ghost" aria-haspopup="dialog" aria-expanded={at !== null}
        onClick={(e) => setAt(at ? null : e.currentTarget.getBoundingClientRect())}>
        <Icon name="gear" size={16} />{p.t("editor.pageSettings")}
      </button>
      {at && (
        <>
          <div className="menu-backdrop" onPointerDown={close} />
          <div ref={pop} className="menu pop" role="dialog" aria-label={p.t("editor.pageSettings")}
            style={{ top: at.bottom + 6, right: Math.max(8, window.innerWidth - at.right) }}
            onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); close(); } }}>
            <PagePanel {...p} />
          </div>
        </>
      )}
    </>
  );
}

/** The open page's own theme and grid size, each either the default or the page's own. */
function PagePanel(p: PagePanelProps) {
  const { page, t } = p;
  const id = useId();
  const [gridError, setGridError] = useState(false);
  useEffect(() => setGridError(false), [page.id]);
  const own = page.grid ?? null;
  const setGrid = (grid: Grid | null) => { const ok = p.onGrid(grid); setGridError(!ok); return ok; };
  const nameOf = (themeId: string) => p.themes.find((th) => th.id === themeId)?.name ?? themeId;
  const size = (g: Grid) => `${g.cols} × ${g.rows}`;

  return (
    <section aria-labelledby={`${id}h`}>
      <h2 id={`${id}h`}>{p.name}</h2>

      <div className="fld">
        <label htmlFor={`${id}t`}>{t("editor.pageTheme")}</label>
        {/* theme names are untrusted free text: plain option text */}
        <select id={`${id}t`} className="in" value={page.theme ?? ""} onFocus={p.onOpenThemes} onPointerDown={p.onOpenThemes}
          onChange={(e) => p.onTheme(e.target.value || null)}>
          <option value="">{t("editor.themeDefault", { name: nameOf(p.defaultTheme) })}</option>
          {p.themes.map((th) => <option key={th.id} value={th.id}>{th.name}</option>)}
          {page.theme && !p.themes.some((th) => th.id === page.theme) && <option value={page.theme}>{page.theme}</option>}
        </select>
      </div>

      <div className="fld">
        <span className="label">{t("editor.pageGrid")}</span>
        <label className="switch">
          <input type="checkbox" role="switch" checked={own === null}
            onChange={(e) => setGrid(e.target.checked ? null : { ...p.defaultGrid })} />
          <span>{t("editor.gridDefault", { size: size(p.defaultGrid) })}</span>
        </label>
        {own && (
          <div className="fields grid-fields">
            <NumField label={t("settings.cols")} value={own.cols} min={1} max={8} t={t} onType onCommit={(n) => setGrid({ cols: n, rows: own.rows })} onBlur={() => setGridError(false)} />
            <NumField label={t("settings.rows")} value={own.rows} min={1} max={8} t={t} onType onCommit={(n) => setGrid({ cols: own.cols, rows: n })} onBlur={() => setGridError(false)} />
          </div>
        )}
        {gridError && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{t("settings.gridNoFit")}</p>}
      </div>
    </section>
  );
}
