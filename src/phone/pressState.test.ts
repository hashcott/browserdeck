import { expect, test } from "vitest";
import { canFire, displayStatus, visiblePressed } from "./pressState";
import type { Layout } from "./PhoneView";

const layout = (ids: string[]) =>
  ({ type: "layout", grid: { cols: 3, rows: 4 }, language: "en", theme: {}, pages: [{ id: "p", name: "", buttons: ids.map((id) => ({ id })) }] }) as unknown as Layout;

test("nothing is pressed unless the connection is open", () => {
  expect(visiblePressed(new Set(["a"]), new Set(["b"]), layout(["a", "b"]), "closed").size).toBe(0);
  expect(visiblePressed(new Set(["a"]), new Set(), layout(["a"]), "connecting").size).toBe(0);
});
test("ids no longer in the layout are dropped; held and pending are merged", () => {
  expect([...visiblePressed(new Set(["a", "gone"]), new Set(["b"]), layout(["a", "b"]), "open")].sort()).toEqual(["a", "b"]);
  expect(visiblePressed(new Set(["a"]), new Set(), null, "open").size).toBe(0);
});
test("non-volume keys ignore taps while pending; volume keys never do", () => {
  const pending = new Set(["k"]);
  expect(canFire("launch", "k", pending)).toBe(false);
  expect(canFire("hotkey", "k", pending)).toBe(false);
  expect(canFire("obs_scene", "k", pending)).toBe(false);
  expect(canFire("launch", "other", pending)).toBe(true);
  expect(canFire("volume", "k", pending)).toBe(true);
});
test("connecting only before the first open; afterwards a drop stays 'closed'", () => {
  expect(displayStatus("connecting", false)).toBe("connecting");
  expect(displayStatus("closed", false)).toBe("connecting");
  expect(displayStatus("connecting", true)).toBe("closed");
  expect(displayStatus("closed", true)).toBe("closed");
  expect(displayStatus("open", true)).toBe("open");
  expect(displayStatus("kicked", true)).toBe("kicked");
});
