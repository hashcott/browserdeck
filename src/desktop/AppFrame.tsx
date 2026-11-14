import type { ReactNode } from "react";
import { Icon } from "./editor/Icons";

export type View = "editor" | "pairing" | "settings";

/** The app shell: the rail, then the open view. The editor stays mounted (hidden) while another view is open,
 *  and the one toast region lives outside that hidden wrapper so a message is visible on every view. */
export function AppFrame({ view, toast, rail, editor, children }: { view: View; toast: string | null; rail?: ReactNode; editor: ReactNode; children?: ReactNode }) {
  return (
    <div className="app">
      {rail}
      <div hidden={view !== "editor"}>{editor}</div>
      {children}
      <div className="toast" role="status" aria-live="polite">{toast && <span><Icon name="alert" size={18} />{toast}</span>}</div>
    </div>
  );
}
