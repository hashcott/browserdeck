import {
  DndContext, DragOverlay, KeyboardSensor, PointerSensor, closestCenter, pointerWithin, useSensor, useSensors,
  type Announcements, type CollisionDetection, type DragEndEvent, type DragStartEvent, type KeyboardCoordinateGetter,
} from "@dnd-kit/core";
import { useEffect, useState, type CSSProperties } from "react";
import type { createT } from "../../shared/i18n";
import { KeyFace, keyName } from "../../shared/KeyFace";
import { keyInk, themeVars } from "../../shared/theme";
import type { ActionType, Button, Config, Grid as GridSize, ObsState, Status, Theme, ThemeInfo } from "../../shared/types";
import { ACTION_TYPES, ActionChip, ActionPalette } from "./ActionPalette";
import { Grid } from "./Grid";
import { Icon, StatusDot } from "./Icons";
import { Inspector, type TestResult } from "./Inspector";
import { PageSettings } from "./PagePanel";
import { PageTabs, pageName } from "./PageTabs";
import "./editor.css";

type T = ReturnType<typeof createT>;
export type Confirm = { kind: "button"; id: string } | { kind: "page"; id: string } | null;
export interface Notice { id: string; text: string }

export interface EditorHandlers {
  selectPage(id: string): void;
  addPage(): void;
  renamePage(id: string, name: string): void;
  askDeletePage(id: string): void;
  deletePage(id: string): void;
  cancelConfirm(): void;
  select(id: string | null): void;
  createAt(slot: number, type: ActionType): void;
  moveTo(id: string, slot: number): void;
  askDeleteButton(id: string): void;
  deleteButton(id: string): void;
  change(b: Button, slow: boolean): void;
  test(id: string): void;
  pickImage(): void;
  pickFile(): void;
  dismissNotice(id: string): void;
  pair(): void;
  obsSettings(): void;
  /** null = the default grid. false = refused (buttons would fall outside). */
  setPageGrid(grid: GridSize | null): boolean;
  /** null = the default theme. */
  setPageTheme(id: string | null): void;
  loadThemes(): void;
}
export interface EditorViewProps {
  config: Config;
  /** The open page's theme (its own, or the default). */
  theme: Theme;
  themes: ThemeInfo[];
  pageId: string;
  selectedId: string | null;
  confirm: Confirm;
  status: Status;
  scenes: string[];
  platform: string;
  notices: Notice[];
  testResult: TestResult | null;
  testing: boolean;
  focusReq: { slot: number; tick: number } | null;
  t: T;
  iconSrc(b: Button): string | undefined;
  on: EditorHandlers;
}

// Keys: Space lifts, arrows pick the neighbouring slot, Enter drops, Escape cancels. (Enter on a key selects it.)
const KEYBOARD_CODES = { start: ["Space"], cancel: ["Escape"], end: ["Enter"] };
const DIRS: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

/** Arrow keys jump to the nearest slot in that direction (same row or column first), centring the dragged item on it. */
const coordinateGetter: KeyboardCoordinateGetter = (event, { context: { droppableRects, droppableContainers, collisionRect } }) => {
  const d = DIRS[event.code];
  if (!d || !collisionRect) return undefined;
  event.preventDefault();
  const cx = collisionRect.left + collisionRect.width / 2, cy = collisionRect.top + collisionRect.height / 2;
  let best: { x: number; y: number; score: number } | null = null;
  for (const c of droppableContainers.getEnabled()) {
    const r = droppableRects.get(c.id);
    if (!r) continue;
    const dx = r.left + r.width / 2 - cx, dy = r.top + r.height / 2 - cy;
    const along = dx * d[0] + dy * d[1];
    if (along < 1) continue;
    const across = Math.abs(dx * d[1]) + Math.abs(dy * d[0]);
    const score = along + 3 * across;
    if (!best || score < best.score) {
      best = { x: r.left + (r.width - collisionRect.width) / 2, y: r.top + (r.height - collisionRect.height) / 2, score };
    }
  }
  return best ? { x: best.x, y: best.y } : undefined;
};

// With a pointer: only the slot under it counts, so dropping elsewhere does nothing. Keyboard: nearest centre.
const collision: CollisionDetection = (args) => (args.pointerCoordinates ? pointerWithin(args) : closestCenter(args));

const slotOf = (id: unknown) => (typeof id === "string" && id.startsWith("slot:") ? Number(id.slice(5)) : null);

export function EditorView(p: EditorViewProps) {
  const { config, theme, t, on } = p;
  const [active, setActive] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  useEffect(() => { setRenaming(false); }, [p.pageId]);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { keyboardCodes: KEYBOARD_CODES, coordinateGetter }),
  );
  const pageIdx = Math.max(0, config.pages.findIndex((pg) => pg.id === p.pageId));
  const page = config.pages[pageIdx];
  const { cols, rows } = page.grid ?? config.settings.grid;
  const selected = page.buttons.find((b) => b.id === p.selectedId) ?? null;
  const confirm = p.confirm;

  const where = (id: unknown) => {
    const s = slotOf(id);
    return s === null ? "" : t("editor.slotPos", { row: String(Math.floor(s / cols) + 1), col: String((s % cols) + 1) });
  };
  const announcements: Announcements = {
    onDragStart: ({ active: a }) => t("editor.dragPicked", { name: dragName(String(a.id)) }),
    onDragOver: ({ over }) => (over ? where(over.id) : undefined),
    onDragEnd: ({ over }) => (over ? t("editor.dragDropped", { pos: where(over.id) }) : t("editor.dragCancelled")),
    onDragCancel: () => t("editor.dragCancelled"),
  };
  const dragName = (id: string) =>
    id.startsWith("new:") ? t(`action.${id.slice(4)}`) : (() => {
      const b = page.buttons.find((x) => x.id === id.slice(4));
      return b ? keyName(b, t) : "";
    })();

  const onDragEnd = ({ active: a, over }: DragEndEvent) => {
    setActive(null);
    const slot = slotOf(over?.id);
    if (slot === null) return;
    const id = String(a.id);
    if (id.startsWith("new:")) on.createAt(slot, id.slice(4) as ActionType);
    else on.moveTo(id.slice(4), slot);
  };
  const overlayButton = active?.startsWith("key:") ? page.buttons.find((b) => `key:${b.id}` === active) : undefined;

  // The active theme's four action colours, for the action chips outside the grid (sidebar, menu, drag overlay).
  const tv = themeVars(theme);
  const actionVars = Object.fromEntries(ACTION_TYPES.flatMap((a) => [[`--c-${a}`, tv[`--c-${a}`]], [`--on-${a}`, keyInk(tv[`--c-${a}`])]])) as CSSProperties;
  const slots = cols * rows;

  return (
    <DndContext sensors={sensors} collisionDetection={collision} accessibility={{ announcements, screenReaderInstructions: { draggable: t("editor.dragHelp") } }}
      onDragStart={(e: DragStartEvent) => setActive(String(e.active.id))} onDragEnd={onDragEnd} onDragCancel={() => setActive(null)}>
      <div className="editor" style={actionVars}>
        {p.notices.length > 0 && (
          <div className="notices">
            {p.notices.map((n) => (
              <div key={n.id} className="notice" role="status">
                <Icon name="alert" size={18} /><span>{n.text}</span>
                <button type="button" className="notice-x" aria-label={t("common.close")} onClick={() => on.dismissNotice(n.id)}><Icon name="close" size={16} /></button>
              </div>
            ))}
          </div>
        )}

        <aside className="side" aria-label={t("app.name")}>
          <div className="side-head">{t("app.name")}</div>
          <div className="side-body">
            <ActionPalette t={t} />
          </div>
          <SideFoot status={p.status} obsAddr={`${config.settings.obs.host}:${config.settings.obs.port}`} t={t} onPair={on.pair} onObsSettings={on.obsSettings} />
        </aside>

        <PageTabs pages={config.pages} pageId={page.id} renaming={renaming} t={t}
          onSelect={on.selectPage} onAdd={on.addPage} onRenameStart={() => setRenaming(true)}
          onRenameEnd={(name) => { setRenaming(false); if (name) on.renamePage(page.id, name); }}
          onAskDelete={(id) => { if (id !== page.id) on.selectPage(id); on.askDeletePage(id); }} />

        <h1 className="sr-only">{pageName(t, page, pageIdx)}</h1>
        {confirm?.kind === "page" ? (
          <div className="toolbar confirm" role="group" aria-label={t("editor.deletePage")} onKeyDown={(e) => e.key === "Escape" && on.cancelConfirm()}>
            <Icon name="trash" size={18} />
            <span>{t("editor.deletePageConfirm", { name: pageName(t, page, pageIdx) })}</span>
            <button type="button" className="btn small danger" onClick={() => on.deletePage(page.id)}>{t("common.delete")}</button>
            <button type="button" className="btn small" autoFocus onClick={on.cancelConfirm}>{t("common.cancel")}</button>
          </div>
        ) : (
          <div className="toolbar">
            <span>{t("editor.slotsUsed", { used: String(page.buttons.length), total: String(slots) })}</span>
            <PageSettings page={page} name={pageName(t, page, pageIdx)} defaultGrid={config.settings.grid} defaultTheme={config.settings.theme}
              themes={p.themes} t={t} onGrid={on.setPageGrid} onTheme={on.setPageTheme} onOpenThemes={on.loadThemes} />
            <button type="button" className="btn small ghost" onClick={() => setRenaming(true)}><Icon name="pencil" size={16} />{t("editor.renamePage")}</button>
          </div>
        )}

        <main className="center" style={themeVars(theme) as CSSProperties} onClick={() => on.select(null)}>
          <div className="stage">
            <Grid page={page} cols={cols} rows={rows} theme={theme} selectedId={selected?.id ?? null} dragging={active !== null}
              focusReq={p.focusReq} t={t} iconSrc={p.iconSrc} onSelect={on.select} onCreate={on.createAt}
              onAskDelete={on.askDeleteButton} />
          </div>
        </main>

        <aside className="right" aria-label={t("editor.properties")}>
          <Inspector button={selected} themeColor={selected ? theme.actions[selected.action.type] : "#000000"}
          presets={ACTION_TYPES.map((type) => ({ type, color: theme.actions[type] })).filter((x) => /^#[0-9a-f]{6}$/i.test(x.color))}
          iconSrc={selected ? p.iconSrc(selected) : undefined} platform={p.platform} obs={p.status.obs} scenes={p.scenes}
          confirming={confirm?.kind === "button" && confirm.id === selected?.id} testing={p.testing} testResult={p.testResult} t={t}
          onChange={on.change} onPickImage={on.pickImage} onPickFile={on.pickFile}
          onTest={() => selected && on.test(selected.id)} onAskDelete={() => selected && on.askDeleteButton(selected.id)}
          onDelete={() => selected && on.deleteButton(selected.id)} onCancelDelete={on.cancelConfirm} />
        </aside>

      </div>

      <DragOverlay dropAnimation={null}>
        {active?.startsWith("new:") && (
          <div className={`act t-${active.slice(4)} overlay-chip`} style={actionVars}><ActionChip type={active.slice(4) as ActionType} t={t} /></div>
        )}
        {overlayButton && (
          <div className="overlay-key" style={themeVars(theme) as CSSProperties}>
            <KeyFace button={overlayButton} theme={theme} iconSrc={p.iconSrc(overlayButton)} />
          </div>
        )}
      </DragOverlay>
    </DndContext>
  );
}

const OBS_TEXT = { connected: "status.obsConnected", disconnected: "status.obsDisconnected", auth_failed: "status.obsAuthFailed", unsupported: "status.obsUnsupported" } satisfies Record<ObsState, string>;

/** The sidebar foot: OBS and phone status in words, and the way to pair while no phone is connected. */
function SideFoot({ status, obsAddr, t, onPair, onObsSettings }: { status: Status; obsAddr: string; t: T; onPair(): void; onObsSettings(): void }) {
  return (
    <div className="side-foot">
      <div className={`foot-row obs-${status.obs}`}>
        <span className="foot-icon"><Icon name="signal" size={20} /></span>
        <span className="foot-text"><b>{t(OBS_TEXT[status.obs])}</b><i>{obsAddr}</i></span>
        {status.obs !== "connected" && (
          <button type="button" className="tool-btn foot-tool" aria-label={t("editor.obsSettings")} title={t("editor.obsSettings")} onClick={onObsSettings}>
            <Icon name="gear" size={18} />
          </button>
        )}
      </div>
      <div className="foot-row">
        <span className="avatar"><Icon name="phone" size={18} /><StatusDot on={status.phone_connected} /></span>
        <span className="foot-text"><b>{t(status.phone_connected ? "status.phoneConnected" : "status.phoneDisconnected")}</b></span>
      </div>
      {!status.phone_connected && (
        <button type="button" className="btn cta small foot-cta" onClick={onPair}><Icon name="phone" size={16} />{t("pairing.title")}</button>
      )}
    </div>
  );
}
