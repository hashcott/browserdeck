import type { ReactNode } from "react";
import { Icon } from "./editor/Icons";

export type View = "editor" | "pairing" | "settings";

/** The app shell: the editor stays mounted (hidden) while another view is open, and the one toast region
 *  lives outside that hidden wrapper so a message is visible on every view. */
export function AppFrame({ view, toast, editor, children }: { view: View; toast: string | null; editor: ReactNode; children?: ReactNode }) {
  return (
    <>
      <div hidden={view !== "editor"}>{editor}</div>
      {children}
      <div className="toast" role="status" aria-live="polite">{toast && <span><Icon name="alert" size={18} />{toast}</span>}</div>
    </>
  );
}
