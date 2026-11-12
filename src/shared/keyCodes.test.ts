import { expect, test } from "vitest";
import keymapRs from "../../src-tauri/src/actions/keymap.rs?raw";
import { SUPPORTED_KEY_CODES } from "./keyCodes";

test("supported key codes match the Rust keymap exactly", () => {
  const body = keymapRs.split("#[cfg(test)]")[0];
  const rust = [...body.matchAll(/"([A-Za-z0-9]+)"\s*=>/g)].map((m) => m[1]);
  expect(rust.length).toBeGreaterThan(50);
  expect([...SUPPORTED_KEY_CODES].sort()).toEqual([...rust].sort());
});
