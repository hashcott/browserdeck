import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import keycap from "../../themes/keycap.json";
import { brandColor, glyphUrl, searchGlyphs, type GlyphEntry } from "./glyphs";
import { KeyFace } from "./KeyFace";
import type { Button, Theme } from "./types";

test("only the two sets and plain names make a URL, since it goes into CSS", () => {
  expect(glyphUrl("ph", "play-pause")).toBe("/glyphs/ph/play-pause.svg");
  expect(glyphUrl("si", "obsstudio")).toBe("/glyphs/si/obsstudio.svg");
  for (const [set, name] of [["xx", "play"], ["ph", "a)b"], ["ph", "../x"], ["ph", 'a"b'], ["ph", ""], ["ph", "Play"]]) {
    expect(glyphUrl(set, name)).toBeUndefined();
  }
});

test("a glyph key is drawn as a mask in the ink colour; a bad name draws nothing", () => {
  const key = (name: string): Button => ({ id: "k", slot: 0, label: "", icon: { kind: "glyph", set: "ph", name }, color: null, action: { type: "volume", op: "up" } });
  expect(renderToStaticMarkup(<KeyFace button={key("speaker-high")} theme={keycap as Theme} />)).toContain("mask-image:url(/glyphs/ph/speaker-high.svg)");
  expect(renderToStaticMarkup(<KeyFace button={key("x);background:url(//evil")} theme={keycap as Theme} />)).not.toContain("mask-image");
});

test("a logo's brand colour becomes the key colour; an icon or a bad value changes nothing", () => {
  expect(brandColor("76B900")).toBe("#76b900"); // NVIDIA
  expect(brandColor(undefined)).toBeNull();
  expect(brandColor("red")).toBeNull();
  expect(brandColor("76B900;x")).toBeNull();
});

test("search puts the closest names first and matches every word; empty shows the starters", () => {
  const e = (name: string, words: string): GlyphEntry => ({ set: "ph", name, title: name, words });
  const all = [e("speaker-x", "speaker x mute audio"), e("speaker-high", "speaker high volume audio"), e("play", "play music"), e("speaker", "speaker audio")];
  expect(searchGlyphs(all, "ph", "speaker").map((g) => g.name)).toEqual(["speaker", "speaker-x", "speaker-high"]);
  expect(searchGlyphs(all, "ph", "audio mute").map((g) => g.name)).toEqual(["speaker-x"]);
  expect(searchGlyphs(all, "ph", "").map((g) => g.name)).toEqual(["speaker-high", "speaker-x"]); // starters, in starter order
  expect(searchGlyphs(all, "si", "speaker")).toEqual([]);
});
