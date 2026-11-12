import { expect, test } from "vitest";
import { finishChord, formatChord, isSupportedKey } from "./HotkeyRecorder";

test("formats a chord readably", () => {
  expect(formatChord(["ControlLeft", "ShiftLeft", "KeyM"], "windows")).toBe("Ctrl + Shift + M");
  expect(formatChord(["MetaLeft", "Digit1", "ArrowUp", "F5"], "macos")).toBe("Cmd + 1 + Up + F5");
  expect(formatChord(["MetaRight", "AltLeft"], "windows")).toBe("Win + Alt");
});

test("only keys the core can send are accepted", () => {
  expect(isSupportedKey("KeyM")).toBe(true);
  for (const c of ["CapsLock", "ContextMenu", "PrintScreen", "AudioVolumeUp", "Unidentified", ""]) expect(isSupportedKey(c)).toBe(false);
});
test("a chord needs a non-modifier key", () => {
  expect(finishChord(["ControlLeft"]).ok).toBe(false);
  expect(finishChord(["ControlLeft", "ShiftRight"]).ok).toBe(false);
  expect(finishChord(["KeyM", "ControlLeft"])).toEqual({ ok: true, keys: ["ControlLeft", "KeyM"] });
  expect(finishChord(["F5"])).toEqual({ ok: true, keys: ["F5"] });
});
