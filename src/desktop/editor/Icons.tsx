import type { ReactNode } from "react";

// One family: 24px grid, 2px stroke, round caps and joins. Interface only; emoji are the user's own icons.
const PATHS = {
  launch: <path d="M5 19 19 5M9 5h10v10" />,
  hotkey: <><rect x="3" y="7" width="18" height="11" rx="2" /><path d="M7 14h10" /></>,
  volume: <path d="M4 10v4h4l5 4V6l-5 4zM17 9a4 4 0 0 1 0 6" />,
  obs_scene: <><circle cx="12" cy="12" r="8" /><circle cx="12" cy="12" r="3" /></>,
  usage: <path d="M3 12h4l3 7 4-14 3 7h4" />,
  plus: <path d="M12 5v14M5 12h14" />,
  trash: <path d="M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13" />,
  pencil: <path d="M5 19l1-4L16 5l3 3L9 18zM14 7l3 3" />,
  check: <path d="M5 12.5 10 17 19 7" />,
  close: <path d="M6 6l12 12M18 6 6 18" />,
  phone: <><rect x="7" y="3" width="10" height="18" rx="2" /><path d="M11 18h2" /></>,
  folder: <path d="M3 7h6l2 2h10v10H3z" />,
  image: <><rect x="3" y="5" width="18" height="14" rx="2" /><circle cx="9" cy="10" r="1.5" /><path d="m4 18 5-5 4 4 3-3 4 4" /></>,
  record: <circle cx="12" cy="12" r="5" />,
  play: <path d="M8 5v14l11-7z" />,
  grip: <path d="M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01" />,
  grid: <><rect x="4" y="4" width="6.5" height="6.5" rx="1.5" /><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5" /><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5" /><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5" /></>,
  signal: <path d="M5 19v-3M10 19v-6M15 19v-9M20 19V7" />,
  gear: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" /></>,
  alert: <path d="M12 4 3 20h18zM12 10v4M12 17h.01" />,
} satisfies Record<string, ReactNode>;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 20 }: { name: IconName; size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" strokeWidth="2"
      strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
      {PATHS[name]}
    </svg>
  );
}

/** Connection state drawn with shape as well as colour: a filled dot is connected, a ring is not. */
export function StatusDot({ on }: { on: boolean }) {
  return (
    <svg className={on ? "dot on" : "dot"} viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
      <circle cx="6" cy="6" r={on ? 5 : 4} fill={on ? "currentColor" : "none"} stroke="currentColor" strokeWidth="2" />
    </svg>
  );
}
