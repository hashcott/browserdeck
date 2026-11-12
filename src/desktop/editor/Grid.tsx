import { useDraggable, useDroppable } from "@dnd-kit/core";
import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import type { createT } from "../../shared/i18n";
import { KeyFace } from "../../shared/KeyFace";
import type { ActionType, Button, Page, Theme } from "../../shared/types";
import { ACTION_TYPES, ActionChip } from "./ActionPalette";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
export interface GridProps {
  page: Page;
  cols: number;
  rows: number;
  theme: Theme;
  selectedId: string | null;
  /** True while a drag is in progress: arrow keys then belong to the drag, not to focus. */
  dragging: boolean;
  /** Ask for keyboard focus on a slot (after create, drop or delete). `tick` makes repeats distinct. */
  focusReq: { slot: number; tick: number } | null;
  t: T;
  iconSrc(b: Button): string | undefined;
  onSelect(id: string | null): void;
  onCreate(slot: number, type: ActionType): void;
  onAskDelete(id: string): void;
}

const ARROWS: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

export function Grid(p: GridProps) {
  const { cols, rows, t } = p;
  const root = useRef<HTMLDivElement>(null);
  const [rawCursor, setCursor] = useState(0);
  const cursor = Math.min(rawCursor, cols * rows - 1); // the grid may shrink; some slot must stay tabbable
  const [menu, setMenu] = useState<{ slot: number; rect: DOMRect } | null>(null);
  const byslot = new Map(p.page.buttons.map((b) => [b.slot, b]));
  const focusSlot = (slot: number) => {
    setCursor(slot);
    root.current?.querySelector<HTMLElement>(`[data-slot="${slot}"] > button`)?.focus();
  };

  useEffect(() => { if (p.focusReq) focusSlot(p.focusReq.slot); }, [p.focusReq]); // eslint-disable-line react-hooks/exhaustive-deps

  const onKey = (e: KeyboardEvent, slot: number) => {
    const d = ARROWS[e.key];
    if (d && !p.dragging) {
      const x = (slot % cols) + d[0], y = Math.floor(slot / cols) + d[1];
      if (x >= 0 && x < cols && y >= 0 && y < rows) focusSlot(y * cols + x);
      e.preventDefault();
    }
  };

  const slots = Array.from({ length: cols * rows }, (_, slot) => {
    const b = byslot.get(slot);
    const common = { slot, cols, tabIndex: slot === cursor ? 0 : -1, onFocus: () => setCursor(slot), onKey: (e: KeyboardEvent) => onKey(e, slot) };
    return b ? (
      <KeySlot key={slot} {...common} button={b} theme={p.theme} selected={b.id === p.selectedId} t={t}
        iconSrc={p.iconSrc(b)} onSelect={p.onSelect} onAskDelete={p.onAskDelete} />
    ) : (
      <EmptySlot key={slot} {...common} t={t} onOpen={(rect) => setMenu({ slot, rect })} />
    );
  });

  return (
    <>
      <div ref={root} className="deck-grid" style={{ "--cols": cols, "--rows": rows } as CSSProperties}>{slots}</div>
      {menu && (
        <SlotMenu rect={menu.rect} t={t}
          onPick={(type) => { setMenu(null); p.onCreate(menu.slot, type); }}
          onClose={() => { setMenu(null); focusSlot(menu.slot); }} />
      )}
    </>
  );
}

interface SlotCommon {
  slot: number;
  cols: number;
  tabIndex: number;
  t: T;
  onFocus(): void;
  onKey(e: KeyboardEvent): void;
}
const pos = (t: T, slot: number, cols: number) =>
  t("editor.slotPos", { row: String(Math.floor(slot / cols) + 1), col: String((slot % cols) + 1) });

function KeySlot(p: SlotCommon & { button: Button; theme: Theme; selected: boolean; iconSrc?: string;
  onSelect(id: string | null): void; onAskDelete(id: string): void }) {
  const { button: b, t } = p;
  const drop = useDroppable({ id: `slot:${p.slot}` });
  const drag = useDraggable({ id: `key:${b.id}` });
  return (
    <div ref={drop.setNodeRef} className={`slot${drop.isOver ? " is-over" : ""}${drag.isDragging ? " is-dragging" : ""}`}
      data-slot={p.slot} style={{ gridColumn: (p.slot % p.cols) + 1, gridRow: Math.floor(p.slot / p.cols) + 1 }}>
      <button type="button" ref={drag.setNodeRef} {...drag.attributes} {...drag.listeners}
        className="key-btn" tabIndex={p.tabIndex} aria-pressed={p.selected}
        aria-label={b.label || t(`action.${b.action.type}`)} aria-description={pos(t, p.slot, p.cols)}
        onFocus={p.onFocus}
        onClick={(e) => { e.stopPropagation(); p.onSelect(b.id); }}
        onKeyDown={(e) => {
          drag.listeners?.onKeyDown?.(e);
          if ((e.key === "Delete" || e.key === "Backspace") && !e.defaultPrevented) { e.preventDefault(); p.onAskDelete(b.id); }
          p.onKey(e);
        }}>
        <KeyFace button={b} theme={p.theme} iconSrc={p.iconSrc} selected={p.selected} />
      </button>
    </div>
  );
}

function EmptySlot(p: SlotCommon & { onOpen(rect: DOMRect): void }) {
  const drop = useDroppable({ id: `slot:${p.slot}` });
  return (
    <div ref={drop.setNodeRef} className={`slot${drop.isOver ? " is-over" : ""}`} data-slot={p.slot}
      style={{ gridColumn: (p.slot % p.cols) + 1, gridRow: Math.floor(p.slot / p.cols) + 1 }}>
      <button type="button" className="empty-btn" tabIndex={p.tabIndex} aria-haspopup="menu"
        aria-label={`${p.t("editor.emptySlot")}, ${pos(p.t, p.slot, p.cols)}`}
        onFocus={p.onFocus} onKeyDown={p.onKey}
        onClick={(e) => { e.stopPropagation(); p.onOpen(e.currentTarget.getBoundingClientRect()); }}>
        <Icon name="plus" size={22} />
      </button>
    </div>
  );
}

const MENU_W = 260, MENU_H = 4 * 62 + 16;

/** The four actions for an empty slot: arrows move, Enter picks, Escape closes. */
function SlotMenu({ rect, t, onPick, onClose }: { rect: DOMRect; t: T; onPick(type: ActionType): void; onClose(): void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { ref.current?.querySelector<HTMLElement>("button")?.focus(); }, []);
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - MENU_W - 8));
  const top = rect.bottom + 6 + MENU_H > window.innerHeight ? Math.max(8, rect.top - 6 - MENU_H) : rect.bottom + 6;
  const onKeyDown = (e: KeyboardEvent) => {
    const items = [...(ref.current?.querySelectorAll<HTMLElement>("button") ?? [])];
    const i = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") items[(i + 1) % items.length].focus();
    else if (e.key === "ArrowUp") items[(i - 1 + items.length) % items.length].focus();
    else if (e.key === "Escape" || e.key === "Tab") onClose();
    else return;
    e.preventDefault();
  };
  return (
    <>
      <div className="menu-backdrop" onPointerDown={onClose} onClick={(e) => e.stopPropagation()} />
      <div ref={ref} className="menu" role="menu" aria-label={t("editor.addButton")} style={{ left, top, width: MENU_W }} onKeyDown={onKeyDown} onClick={(e) => e.stopPropagation()}>
        {ACTION_TYPES.map((a) => (
          <button key={a} type="button" role="menuitem" className={`act t-${a}`} onClick={() => onPick(a)}>
            <ActionChip type={a} t={t} />
          </button>
        ))}
      </div>
    </>
  );
}
