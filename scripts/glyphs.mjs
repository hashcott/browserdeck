// Copies the key icon sets into public/glyphs (served by Vite in dev, bundled into dist on build):
//   ph/<name>.svg  Phosphor, Fill weight (MIT, its licence goes along)
//   si/<slug>.svg  Simple Icons brand logos (CC0)
//   ph.json [[name, "search words"]], si.json [[slug, title, hex]] for the icon picker.
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { icons } from "@phosphor-icons/core";

const out = new URL("../public/glyphs/", import.meta.url);
const ph = new URL("../node_modules/@phosphor-icons/core/", import.meta.url);
const si = new URL("../node_modules/simple-icons/", import.meta.url);

rmSync(out, { recursive: true, force: true });
mkdirSync(new URL("ph/", out), { recursive: true });
mkdirSync(new URL("si/", out), { recursive: true });

for (const f of readdirSync(new URL("assets/fill/", ph))) {
  if (f.endsWith("-fill.svg")) copyFileSync(new URL(`assets/fill/${f}`, ph), new URL(`ph/${f.replace("-fill.svg", ".svg")}`, out));
}
copyFileSync(new URL("LICENSE", ph), new URL("ph/LICENSE.txt", out));
const words = (i) => [...new Set([...i.name.split("-"), ...i.tags.filter((t) => !t.startsWith("*")), ...i.categories])].join(" ");
writeFileSync(new URL("ph.json", out), JSON.stringify(icons.map((i) => [i.name, words(i)])));

const brands = JSON.parse(readFileSync(new URL("data/simple-icons.json", si), "utf8"));
for (const b of brands) copyFileSync(new URL(`icons/${b.slug}.svg`, si), new URL(`si/${b.slug}.svg`, out));
writeFileSync(new URL("si.json", out), JSON.stringify(brands.map((b) => [b.slug, b.title, b.hex])));

console.log(`glyphs: ${icons.length} Phosphor icons, ${brands.length} Simple Icons logos`);
