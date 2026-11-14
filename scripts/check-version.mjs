// Release guard: the pushed tag must be "v" + the version that package.json, tauri.conf.json and Cargo.toml all share.
// Usage: node scripts/check-version.mjs v0.1.0   (exit 0 and prints the version, or exit 1 and says what differs)
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const versions = {
  "package.json": JSON.parse(read("package.json")).version,
  "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
  "src-tauri/Cargo.toml": /^version\s*=\s*"([^"]+)"/m.exec(read("src-tauri/Cargo.toml"))?.[1],
};
const tag = process.argv[2] ?? "";
const wanted = tag.startsWith("v") ? tag.slice(1) : null;
const wrong = Object.entries(versions).filter(([, v]) => v !== wanted);

if (wanted && wrong.length === 0) {
  console.log(wanted);
} else {
  console.error(`Tag "${tag}" must be "v" followed by the app version. Versions found:`);
  for (const [file, v] of Object.entries(versions)) console.error(`  ${file}: ${v}`);
  process.exit(1);
}
