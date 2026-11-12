import type { createT } from "../shared/i18n";
import { Icon } from "./editor/Icons";

/** Top bar of the non-editor screens: same navy bar as the editor, with a way back. */
export function ScreenBar({ title, backLabel, onBack, t }: { title: string; backLabel: string; onBack(): void; t: ReturnType<typeof createT> }) {
  return (
    <header className="bar">
      <div className="logo">{t("app.name")}</div>
      <button type="button" className="btn small ghost-on-bar" onClick={onBack}><Icon name="back" size={18} />{backLabel}</button>
      <h1 className="screen-title">{title}</h1>
    </header>
  );
}
