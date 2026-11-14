import { useDraggable } from "@dnd-kit/core";
import type { createT } from "../../shared/i18n";
import type { ActionType } from "../../shared/types";
import { Icon } from "./Icons";

type T = ReturnType<typeof createT>;
export const ACTION_TYPES: ActionType[] = ["launch", "hotkey", "volume", "media", "obs_scene", "usage"];

/** The row's look, shared with the drag overlay. */
export function ActionChip({ type, t }: { type: ActionType; t: T }) {
  return (
    <>
      <span className="ai"><Icon name={type} size={22} /></span>
      <span><b>{t(`action.${type}`)}</b><i>{t(`action.${type}.desc`)}</i></span>
    </>
  );
}

function Row({ type, t }: { type: ActionType; t: T }) {
  const d = useDraggable({ id: `new:${type}` });
  return (
    <li>
      <button type="button" ref={d.setNodeRef} {...d.attributes} {...d.listeners}
        className={`act t-${type}${d.isDragging ? " is-dragging" : ""}`}>
        <ActionChip type={type} t={t} />
      </button>
    </li>
  );
}

export function ActionPalette({ t }: { t: T }) {
  return (
    <section aria-labelledby="actions-h">
      <div className="cat"><h2 id="actions-h">{t("editor.actions")}</h2></div>
      <ul className="acts">{ACTION_TYPES.map((a) => <Row key={a} type={a} t={t} />)}</ul>
      <p className="note">{t("editor.dragHint")}</p>
    </section>
  );
}
