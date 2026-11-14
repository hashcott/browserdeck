import type { ReactNode } from "react";
import type { createT } from "../shared/i18n";
import type { View } from "./AppFrame";
import { Icon, StatusDot } from "./editor/Icons";

type T = ReturnType<typeof createT>;

/** The small cut of the reversed symbol: the rail button is under 48px. */
function Logo() {
  return (
    <svg viewBox="0 0 256 256" width="30" height="30" aria-hidden="true" focusable="false">
      <path fill="#F4F3FF" d="M66 60h40V235H66zM66 167a70 70 0 1 1 140 0a70 70 0 1 1 -140 0zM106 167a30 30 0 1 0 60 0a30 30 0 1 0 -60 0z" />
      <path fill="#FFD400" d="M106 167a30 30 0 1 1 60 0a30 30 0 1 1 -60 0z" />
      <path fill="#FF4040" d="M50 55a36 36 0 1 1 72 0a36 36 0 1 1 -72 0z" />
    </svg>
  );
}

/** The far-left rail: one button per view, a round icon over a short label. A pill on the left edge marks the open view. */
export function Rail({ view, phoneConnected, t, onNavigate }: { view: View; phoneConnected: boolean; t: T; onNavigate(v: View): void }) {
  // The visible label is short; the accessible name is the full one (it contains the label) plus any status.
  const item = (v: View, label: string, short: string, icon: ReactNode, status?: { on: boolean; text: string }) => (
    <li className={v === view ? "rail-item on" : "rail-item"}>
      <button type="button" className="rail-btn" data-nav={v}
        aria-label={status ? `${label}, ${status.text}` : label} aria-current={v === view ? "page" : undefined}
        onClick={() => onNavigate(v)}>
        <span className="rail-ic">
          {icon}
          {status && <span className="rail-badge"><StatusDot on={status.on} /></span>}
        </span>
        <span className="rail-label">{short}</span>
      </button>
    </li>
  );
  return (
    <nav className="rail" aria-label={t("app.name")}>
      <ul>
        {item("editor", t("rail.editor"), t("rail.editor"), <Logo />)}
        <li className="rail-sep" aria-hidden="true" />
        {item("pairing", t("pairing.title"), t("rail.phone"), <Icon name="phone" size={22} />,
          { on: phoneConnected, text: t(phoneConnected ? "status.phoneConnected" : "status.phoneDisconnected") })}
        {item("settings", t("settings.title"), t("settings.title"), <Icon name="gear" size={22} />)}
      </ul>
    </nav>
  );
}
