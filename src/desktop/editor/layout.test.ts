import { expect, test } from "vitest";
import type { Button, Page } from "../../shared/types";
import { defaultAction, fitsGrid, moveButton, newButton, placeNew, removeButton, updateButton } from "./layout";

const btn = (id: string, slot: number): Button => newButton(id, slot, defaultAction("volume"));
const page = (...buttons: Button[]): Page => ({ id: "p", name: "P", buttons });
const slotOf = (p: Page, id: string) => p.buttons.find((b) => b.id === id)?.slot;

function deepFreeze<T>(o: T): T {
  if (o && typeof o === "object") {
    Object.values(o).forEach(deepFreeze);
    Object.freeze(o);
  }
  return o;
}

test("placeNew adds a button to an empty slot", () => {
  const p = placeNew(page(btn("a", 0)), 2, btn("b", 2));
  expect(p.buttons.map((b) => b.id)).toEqual(["a", "b"]);
});
test("placeNew on an occupied slot changes nothing", () => {
  const before = page(btn("a", 0));
  expect(placeNew(before, 0, btn("b", 0))).toBe(before);
});
test("moveButton to an empty slot moves it", () => {
  expect(slotOf(moveButton(page(btn("a", 0)), "a", 5), "a")).toBe(5);
});
test("moveButton onto another button swaps the two", () => {
  const p = moveButton(page(btn("a", 0), btn("b", 3)), "a", 3);
  expect(slotOf(p, "a")).toBe(3);
  expect(slotOf(p, "b")).toBe(0);
});
test("moveButton to its own slot changes nothing", () => {
  const before = page(btn("a", 1));
  expect(moveButton(before, "a", 1)).toBe(before);
});
test("removeButton removes only that button", () => {
  expect(removeButton(page(btn("a", 0), btn("b", 1)), "a").buttons.map((b) => b.id)).toEqual(["b"]);
});
test("updateButton replaces the button with the same id", () => {
  const p = updateButton(page(btn("a", 0)), { ...btn("a", 0), label: "x" });
  expect(p.buttons[0].label).toBe("x");
});
test("functions do not mutate their input", () => {
  const p = deepFreeze(page(btn("a", 0), btn("b", 3)));
  expect(() => {
    placeNew(p, 5, btn("c", 5));
    moveButton(p, "a", 3);
    moveButton(p, "a", 7);
    removeButton(p, "a");
    updateButton(p, { ...p.buttons[0], label: "x" });
  }).not.toThrow();
});
test("fitsGrid is false when a button would fall outside", () => {
  const pages = [page(btn("a", 11))];
  expect(fitsGrid(pages, { cols: 3, rows: 3 })).toBe(false);
  expect(fitsGrid(pages, { cols: 3, rows: 4 })).toBe(true);
});
test("newButton uses the default emoji per action type and leaves colour to the theme", () => {
  const emoji = { launch: "🚀", hotkey: "⌨️", volume: "🔊", obs_scene: "🎬" } as const;
  for (const [type, value] of Object.entries(emoji)) {
    const b = newButton("i", 0, defaultAction(type as keyof typeof emoji));
    expect(b.icon).toEqual({ kind: "emoji", value });
    expect(b.color).toBeNull();
    expect(b.label).toBe("");
  }
});
test("defaultAction values", () => {
  expect(defaultAction("launch")).toEqual({ type: "launch", path: "", args: [], admin: false });
  expect(defaultAction("hotkey")).toEqual({ type: "hotkey", keys: [] });
  expect(defaultAction("volume")).toEqual({ type: "volume", op: "up" });
  expect(defaultAction("obs_scene")).toEqual({ type: "obs_scene", scene: "" });
});
