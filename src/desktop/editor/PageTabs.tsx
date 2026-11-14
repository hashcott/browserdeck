import { useEffect, useRef } from "react";
import type { createT } from "../../shared/i18n";
import type { Page } from "../../shared/types";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
export interface PageTabsProps {
  pages: Page[];
  pageId: string;
  /** The open page's tab shows a name field. */
  renaming: boolean;
  t: T;
  onSelect(id: string): void;
  onAdd(): void;
  onRenameStart(): void;
  onRenameEnd(name: string | null): void;
  /** The × on a tab: opens that page and asks to delete it. */
  onAskDelete(id: string): void;
}

export const pageName = (t: T, p: Page, i: number) => p.name || t("phone.page", { n: String(i + 1) });

/** The pages as browser tabs above the grid. Double-click or F2 renames the open one; × asks to delete. */
export function PageTabs(p: PageTabsProps) {
  const { pages, pageId, t } = p;
  const strip = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const current = pages.find((pg) => pg.id === pageId) ?? pages[0];
  useEffect(() => { strip.current?.querySelector(".tab.on")?.scrollIntoView({ block: "nearest", inline: "nearest" }); }, [pageId, pages.length]);
  useEffect(() => { if (p.renaming) input.current?.select(); }, [p.renaming]);

  const commit = (value: string) => {
    const name = value.trim();
    p.onRenameEnd(name && name !== current.name ? name : null);
  };

  return (
    <nav className="tabs" aria-label={t("phone.pages")}>
      <div className="tabs-scroll" ref={strip}>
        {pages.map((pg, i) => {
          const on = pg.id === current.id;
          return (
            <div key={pg.id} className={on ? "tab on" : "tab"}>
              {on && p.renaming ? (
                <input ref={input} className="tab-input" defaultValue={pg.name} maxLength={40} aria-label={t("editor.renamePage")}
                  onBlur={(e) => commit(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commit(e.currentTarget.value);
                    if (e.key === "Escape") { e.currentTarget.value = current.name; p.onRenameEnd(null); } // a blur while unmounting then sees no change
                  }} />
              ) : (
                <button type="button" className="tab-btn" aria-current={on ? "page" : undefined} title={on ? t("editor.renameHint") : undefined}
                  aria-keyshortcuts={on ? "F2" : undefined}
                  onClick={() => p.onSelect(pg.id)}
                  onDoubleClick={() => on && p.onRenameStart()}
                  onKeyDown={(e) => { if (e.key === "F2" && on) { e.preventDefault(); p.onRenameStart(); } }}>
                  <Icon name="grid" size={16} /><span>{pageName(t, pg, i)}</span>
                </button>
              )}
              {pages.length > 1 && !(on && p.renaming) && (
                <button type="button" className="tab-x" aria-label={`${t("editor.deletePage")}: ${pageName(t, pg, i)}`} title={t("editor.deletePage")}
                  onClick={() => p.onAskDelete(pg.id)}>
                  <Icon name="close" size={14} />
                </button>
              )}
            </div>
          );
        })}
      </div>
      <button type="button" className="tab-add" aria-label={t("editor.addPage")} title={t("editor.addPage")} onClick={p.onAdd}>
        <Icon name="plus" size={18} />
      </button>
    </nav>
  );
}
