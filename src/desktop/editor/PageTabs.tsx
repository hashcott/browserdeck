import { useEffect, useRef, useState } from "react";
import type { createT } from "../../shared/i18n";
import type { Page } from "../../shared/types";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
export interface PageTabsProps {
  pages: Page[];
  pageId: string;
  confirmDelete: boolean;
  t: T;
  onSelect(id: string): void;
  onAdd(): void;
  onRename(id: string, name: string): void;
  onAskDelete(): void;
  onDelete(): void;
  onCancel(): void;
}

export const pageName = (t: T, p: Page, i: number) => p.name || t("phone.page", { n: String(i + 1) });

export function PageTabs(p: PageTabsProps) {
  const { pages, pageId, t } = p;
  const [renaming, setRenaming] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const idx = Math.max(0, pages.findIndex((pg) => pg.id === pageId));
  const current = pages[idx];
  useEffect(() => { setRenaming(false); }, [pageId]);
  useEffect(() => { if (renaming) input.current?.select(); }, [renaming]);

  if (p.confirmDelete) {
    return (
      <div className="tabs confirm" role="group" onKeyDown={(e) => e.key === "Escape" && p.onCancel()}>
        <span>{t("editor.deletePageConfirm", { name: pageName(t, current, idx) })}</span>
        <button type="button" className="btn small danger-on-bar" autoFocus onClick={p.onDelete}>{t("common.delete")}</button>
        <button type="button" className="btn small ghost-on-bar" onClick={p.onCancel}>{t("common.cancel")}</button>
      </div>
    );
  }

  const commit = (value: string) => {
    setRenaming(false);
    const name = value.trim();
    if (name && name !== current.name) p.onRename(current.id, name);
  };

  return (
    <nav className="tabs" aria-label={t("phone.pages")}>
      {pages.map((pg, i) =>
        pg.id === current.id && renaming ? (
          <input key={pg.id} ref={input} className="tab-input" defaultValue={pg.name} maxLength={40}
            aria-label={t("editor.renamePage")}
            onBlur={(e) => commit(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit(e.currentTarget.value);
              if (e.key === "Escape") { e.currentTarget.value = current.name; setRenaming(false); } // a blur while unmounting then sees no change
            }} />
        ) : (
          <button key={pg.id} type="button" className={pg.id === current.id ? "tab on" : "tab"}
            aria-current={pg.id === current.id ? "page" : undefined} onClick={() => p.onSelect(pg.id)}>
            {pageName(t, pg, i)}
          </button>
        ))}
      <button type="button" className="tab icon-only" aria-label={t("editor.addPage")} title={t("editor.addPage")} onClick={p.onAdd}>
        <Icon name="plus" size={18} />
      </button>
      <span className="tab-tools">
        <button type="button" className="tab icon-only" aria-label={t("editor.renamePage")} title={t("editor.renamePage")} onClick={() => setRenaming(true)}>
          <Icon name="pencil" size={16} />
        </button>
        <button type="button" className="tab icon-only" aria-label={t("editor.deletePage")} title={t("editor.deletePage")}
          disabled={pages.length < 2} onClick={p.onAskDelete}>
          <Icon name="trash" size={16} />
        </button>
      </span>
    </nav>
  );
}
