import { expect, test } from "vitest";
import { formatChord } from "./HotkeyRecorder";

test("formats a chord readably", () => {
  expect(formatChord(["ControlLeft", "ShiftLeft", "KeyM"], "windows")).toBe("Ctrl + Shift + M");
  expect(formatChord(["MetaLeft", "Digit1", "ArrowUp", "F5"], "macos")).toBe("Cmd + 1 + Up + F5");
  expect(formatChord(["MetaRight", "AltLeft"], "windows")).toBe("Win + Alt");
});
