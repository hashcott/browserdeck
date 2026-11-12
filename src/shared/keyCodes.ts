// Exactly the KeyboardEvent.code values src-tauri/src/actions/keymap.rs can map; keyCodes.test.ts keeps them in step.
export const SUPPORTED_KEY_CODES: readonly string[] = [
  "Escape", "Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
  "Digit9", "Digit0", "Minus", "Equal", "Backspace", "Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY",
  "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight", "Backslash", "KeyA", "KeyS", "KeyD", "KeyF",
  "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote", "Enter", "IntlBackslash", "KeyZ", "KeyX",
  "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash", "Space", "ControlLeft", "ShiftLeft",
  "AltLeft", "MetaLeft", "ControlRight", "ShiftRight", "AltRight", "MetaRight", "F1", "F2", "F3", "F4", "F5",
  "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15", "F16", "F17", "F18", "F19", "F20", "F21",
  "F22", "F23", "F24", "ArrowUp", "ArrowLeft", "ArrowRight", "ArrowDown", "Insert", "Delete", "Home", "End",
  "PageUp", "PageDown", "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6",
  "Numpad7", "Numpad8", "Numpad9", "NumpadDecimal", "NumpadAdd", "NumpadSubtract", "NumpadMultiply",
  "NumpadDivide", "NumpadEnter",
];
