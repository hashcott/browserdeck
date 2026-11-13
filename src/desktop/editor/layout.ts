import type { Action, Button, Grid, Page } from "../../shared/types";

const EMOJI: Record<Action["type"], string> = { launch: "🚀", hotkey: "⌨️", volume: "🔊", obs_scene: "🎬" };

export function defaultAction(type: Action["type"]): Action {
  switch (type) {
    case "launch": return { type, path: "", args: [], admin: false };
    case "hotkey": return { type, keys: [] };
    case "volume": return { type, op: "up" };
    case "obs_scene": return { type, scene: "" };
  }
}

export function newButton(id: string, slot: number, action: Action): Button {
  return { id, slot, label: "", icon: { kind: "emoji", value: EMOJI[action.type] }, color: null, action };
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
