import type { Action, Button, Grid, Icon, MediaOp, Page, VolumeOp } from "../../shared/types";

// Phosphor names: a new key's icon, per action type, and per operation for volume and media keys.
const GLYPH: Record<Action["type"], string> = { launch: "rocket-launch", hotkey: "keyboard", volume: "speaker-high", obs_scene: "broadcast", usage: "cpu", media: "play-pause" };
const OP_GLYPH: Record<VolumeOp | MediaOp, string> = { up: "speaker-high", down: "speaker-low", mute: "speaker-x", previous: "skip-back", play_pause: "play-pause", next: "skip-forward" };

export function defaultIcon(a: Action): Icon {
  return { kind: "glyph", set: "ph", name: a.type === "volume" || a.type === "media" ? OP_GLYPH[a.op] : GLYPH[a.type] };
}

export const sameIcon = (a: Icon, b: Icon): boolean => JSON.stringify(a) === JSON.stringify(b);

export function defaultAction(type: Action["type"]): Action {
  switch (type) {
    case "launch": return { type, path: "", args: [], admin: false };
    case "hotkey": return { type, keys: [] };
    case "volume": return { type, op: "up" };
    case "obs_scene": return { type, scene: "" };
    case "usage": return { type, metric: "cpu" };
    case "media": return { type, op: "play_pause" };
  }
}

export function newButton(id: string, slot: number, action: Action): Button {
  return { id, slot, label: "", icon: defaultIcon(action), color: null, action };
}

/** What dropping an action on an empty slot creates. Media makes a player row (previous, play/pause, next)
 *  when the two slots to its right, in the same row, are free too; otherwise a single play/pause key. */
export function newButtons(page: Page, slot: number, type: Action["type"], grid: Grid, id: () => string): Button[] {
  if (type !== "media") return [newButton(id(), slot, defaultAction(type))];
  const free = (s: number) => s < grid.cols * grid.rows && !page.buttons.some((b) => b.slot === s);
  if (slot % grid.cols > grid.cols - 3 || !free(slot + 1) || !free(slot + 2)) return [newButton(id(), slot, defaultAction(type))];
  return (["previous", "play_pause", "next"] as const).map((op, i) => newButton(id(), slot + i, { type, op }));
}

/** An occupied slot returns the same page object. */
export function placeNew(page: Page, slot: number, button: Button): Page {
  if (page.buttons.some((b) => b.slot === slot)) return page;
  return { ...page, buttons: [...page.buttons, { ...button, slot }] };
}

/** Moving onto another button swaps the two. */
export function moveButton(page: Page, buttonId: string, toSlot: number): Page {
  const from = page.buttons.find((b) => b.id === buttonId);
  if (!from || from.slot === toSlot) return page;
  return {
    ...page,
    buttons: page.buttons.map((b) =>
      b.id === buttonId ? { ...b, slot: toSlot } : b.slot === toSlot ? { ...b, slot: from.slot } : b),
  };
}

export const removeButton = (page: Page, buttonId: string): Page =>
  ({ ...page, buttons: page.buttons.filter((b) => b.id !== buttonId) });

export const updateButton = (page: Page, button: Button): Page =>
  ({ ...page, buttons: page.buttons.map((b) => (b.id === button.id ? button : b)) });

export const fitsGrid = (pages: Page[], grid: Grid): boolean =>
  pages.every((p) => p.buttons.every((b) => b.slot < grid.cols * grid.rows));
