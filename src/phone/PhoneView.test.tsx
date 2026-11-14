import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import arcade from "../../themes/arcade.json";
import keycap from "../../themes/keycap.json";
import { createT } from "../shared/i18n";
import type { Button, Theme } from "../shared/types";
import { PhoneView, type Layout } from "./PhoneView";

const btn = (id: string, slot: number): Button =>
  ({ id, slot, label: id, icon: { kind: "emoji", value: "" }, color: null, action: { type: "volume", op: "up" } });
const render = (layout: Layout) => renderToStaticMarkup(
  <PhoneView layout={layout} status="open" pressed={new Set()} errors={[]} t={createT("en")}
    onDown={() => {}} onUp={() => {}} onPress={() => {}} onReload={() => {}} />);

test("each page is drawn on its own grid and theme, and keys outside its grid are left out", () => {
  const html = render({
    type: "layout", language: "en", grid: { cols: 3, rows: 4 }, theme: keycap as Theme,
    pages: [
      { id: "a", name: "A", buttons: [btn("a1", 0)], grid: { cols: 3, rows: 4 }, theme: keycap as Theme },
      { id: "b", name: "B", buttons: [btn("b1", 9), btn("b2", 10)], grid: { cols: 5, rows: 2 }, theme: arcade as Theme },
    ],
  });
  expect(html).toContain("--cols:3;--rows:4");
  expect(html).toContain("--cols:5;--rows:2");
  expect(html).toContain(`--ground:${arcade.ground}`);
  expect(html).toContain('aria-label="b1"'); // slot 9 fits 5 x 2
  expect(html).not.toContain('aria-label="b2"'); // slot 10 does not
});
