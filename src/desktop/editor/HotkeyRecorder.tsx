import { useEffect, useRef, useState } from "react";
import { SUPPORTED_KEY_CODES } from "../../shared/keyCodes";
import { Icon } from "./Icons";

const MODS = ["Control", "Alt", "Shift", "Meta"];
const isMod = (code: string) => MODS.some((m) => code.startsWith(m));
const NAMES: Record<string, string> = {
  Space: "Space", Enter: "Enter", Escape: "Esc", Backspace: "Backspace", Tab: "Tab", Delete: "Delete",
  Insert: "Insert", Home: "Home", End: "End", PageUp: "Page Up", PageDown: "Page Down",
  ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
  Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\",
  Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", IntlBackslash: "\\",
  NumpadDecimal: "Num .", NumpadAdd: "Num +", NumpadSubtract: "Num -", NumpadMultiply: "Num *",
  NumpadDivide: "Num /", NumpadEnter: "Num Enter",
};

/** "ControlLeft" + "KeyM" -> "Ctrl + M". `platform` is std::env::consts::OS. */
export function formatKey(code: string, platform: string): string {
  if (code.startsWith("Control")) return "Ctrl";
  if (code.startsWith("Alt")) return platform === "macos" ? "Option" : "Alt";
  if (code.startsWith("Shift")) return "Shift";
  if (code.startsWith("Meta")) return platform === "macos" ? "Cmd" : "Win";
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return `Num ${code.slice(6)}`;
  return NAMES[code] ?? code;
}
export const formatChord = (keys: string[], platform: string) => keys.map((k) => formatKey(k, platform)).join(" + ");

/** Modifiers first (the core presses in order), otherwise the order the keys went down. */
const ordered = (codes: string[]) => [...codes.filter(isMod), ...codes.filter((c) => !isMod(c))];

/** Whether a pressed key may be part of a chord: only codes the core can send. */
export const isSupportedKey = (code: string) => SUPPORTED_KEY_CODES.includes(code);

/** Whether a finished recording may be saved: it needs at least one non-modifier key. */
export function finishChord(seen: string[]): { ok: true; keys: string[] } | { ok: false } {
  return seen.some((c) => !isMod(c)) ? { ok: true, keys: ordered(seen) } : { ok: false };
}

interface Props {
  keys: string[];
  platform: string;
  t: (key: string) => string;
  onChange(keys: string[]): void;
}

/** Records the codes held down until every key is released. Listens only while recording; Escape cancels. */
export function HotkeyRecorder({ keys, platform, t, onChange }: Props) {
  const [recording, setRecording] = useState(false);
  const [live, setLive] = useState<string[]>([]);
  const [hint, setHint] = useState<string | null>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const emit = useRef(onChange);
  emit.current = onChange; // the listeners below must survive parent re-renders

  useEffect(() => {
    if (!recording) return;
    const seen: string[] = [];
    const held = new Set<string>();
    const stop = () => { setRecording(false); setLive([]); setHint(null); };
    const down = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") return stop();
      if (e.repeat || !e.code) return;
      if (!isSupportedKey(e.code)) return setHint("editor.hotkeyUnsupported");
      setHint(null);
      held.add(e.code);
      if (!seen.includes(e.code)) seen.push(e.code);
      setLive(ordered(seen));
    };
    const up = (e: KeyboardEvent) => {
      e.preventDefault();
      // macOS swallows the keyup of other keys while Cmd is down, so releasing Cmd ends the chord.
      if (e.code.startsWith("Meta")) held.clear(); else held.delete(e.code);
      if (held.size === 0 && seen.length > 0) {
        const r = finishChord(seen);
        if (r.ok) {
          emit.current(r.keys);
          stop();
          btn.current?.focus();
        } else { // modifiers only: explain and keep listening
          seen.length = 0;
          setLive([]);
          setHint("editor.hotkeyNeedsKey");
        }
      }
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
      window.removeEventListener("blur", stop);
    };
  }, [recording]);

  const shown = recording ? live : keys;
  return (
    <div className="hotkey">
      <div className="chord" aria-live="polite">
        {shown.length > 0 ? formatChord(shown, platform) : <span className="hint">{t(recording ? "editor.hotkeyPress" : "editor.hotkeyNone")}</span>}
      </div>
      <div className="note hotkey-hint" role="status">{recording && hint ? t(hint) : ""}</div>
      <button ref={btn} type="button" className={recording ? "btn small recording" : "btn small"}
        onClick={() => setRecording((r) => !r)} onBlur={() => setRecording(false)}>
        <Icon name="record" size={16} />
        {t(recording ? "editor.hotkeyCancel" : "editor.hotkeyRecord")}
      </button>
    </div>
  );
}
