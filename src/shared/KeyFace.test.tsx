import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import keycapJson from "../../themes/keycap.json";
import { KeyFace } from "./KeyFace";
import type { Button, Theme } from "./types";

const theme = keycapJson as Theme;
const button: Button = {
  id: "b", slot: 0, label: "Mic", icon: { kind: "emoji", value: "🎙️" }, color: null,
  action: { type: "hotkey", keys: ["KeyM"] },
};

test("classes come from the theme's fixed sets, plus state flags", () => {
  const html = renderToStaticMarkup(<KeyFace button={button} theme={theme} pressed selected />);
  expect(html).toContain("depth-dish press-sink legend-top_left");
  expect(html).toContain("is-pressed");
  expect(html).toContain("is-selected");
});
test("a label with markup is escaped", () => {
  const html = renderToStaticMarkup(<KeyFace button={{ ...button, label: "<script>x</script>" }} theme={theme} />);
  expect(html).not.toContain("<script>");
  expect(html).toContain("&lt;script&gt;");
});
test("inline style holds only custom properties", () => {
  const html = renderToStaticMarkup(<KeyFace button={button} theme={theme} />);
  const style = /style="([^"]*)"/.exec(html)?.[1] ?? "";
  const decls = style.split(";").filter(Boolean);
  expect(decls.length).toBeGreaterThan(5);
  expect(decls.every((d) => d.startsWith("--"))).toBe(true);
});
test("an image icon uses the caller's src, with empty alt", () => {
  const img = { ...button, icon: { kind: "image" as const, file: "a.png" } };
  expect(renderToStaticMarkup(<KeyFace button={img} theme={theme} iconSrc="/i/a.png" />)).toContain('src="/i/a.png" alt=""');
});
