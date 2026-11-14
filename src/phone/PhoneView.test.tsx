import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import arcade from "../../themes/arcade.json";
import keycap from "../../themes/keycap.json";
import { createT } from "../shared/i18n";
import type { Button, Theme, Usage } from "../shared/types";
import { PhoneView, type Layout } from "./PhoneView";

const btn = (id: string, slot: number): Button =>
  ({ id, slot, label: id, icon: { kind: "emoji", value: "" }, color: null, action: { type: "volume", op: "up" } });
const render = (layout: Layout, usage: Usage | null = null) => renderToStaticMarkup(
  <PhoneView layout={layout} usage={usage} status="open" pressed={new Set()} errors={[]} t={createT("en")}
    onDown={() => {}} onUp={() => {}} onPress={() => {}} onReload={() => {}} />);

test("each page is drawn on its own grid and theme, without its empty trailing rows or keys outside its grid", () => {
  const html = render({
    type: "layout", language: "en", grid: { cols: 3, rows: 4 }, theme: keycap as Theme, build: "",
    pages: [
      { id: "a", name: "A", buttons: [btn("a1", 0), btn("a2", 4)], grid: { cols: 3, rows: 4 }, theme: keycap as Theme },
      { id: "b", name: "B", buttons: [btn("b1", 9), btn("b2", 10)], grid: { cols: 5, rows: 2 }, theme: arcade as Theme },
    ],
  });
  expect(html).toContain("--cols:3;--rows:2"); // slot 4 is in row 2; rows 3 and 4 are empty
  expect(html).toContain('class="deck-grid wide"'); // 3 x 2 is turned on a portrait screen
  expect(html).toContain("--cols:5;--rows:2");
  expect(html).toContain(`--ground:${arcade.ground}`);
  expect(html).toContain('aria-label="b1"'); // slot 9 fits 5 x 2
  expect(html).not.toContain('aria-label="b2"'); // slot 10 does not
});

test("a usage key shows the latest reading, or a dash before one arrives", () => {
  const usageBtn = (id: string, slot: number, metric: "cpu" | "temp"): Button => ({ ...btn(id, slot), label: "", action: { type: "usage", metric } });
  const layout: Layout = {
    type: "layout", language: "en", grid: { cols: 3, rows: 4 }, theme: keycap as Theme, build: "",
    pages: [{ id: "a", name: "A", buttons: [usageBtn("c", 0, "cpu"), usageBtn("t", 1, "temp")], grid: { cols: 3, rows: 4 }, theme: keycap as Theme }],
  };
  expect(render(layout)).toContain('aria-label="CPU —"');
  const html = render(layout, { cpu: 37, ram: 52, temp: null });
  expect(html).toContain('aria-label="CPU 37%"');
  expect(html).toContain('<span class="key-value">37%</span>');
  expect(html).toContain('aria-label="Temp —"'); // this PC reports no temperature
});
