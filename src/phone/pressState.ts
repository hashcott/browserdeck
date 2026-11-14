import type { ActionType } from "../shared/types";
import type { ConnStatus } from "./connection";
import type { Layout } from "./PhoneView";

/** Held and pending keys that may be drawn pressed: none unless open, and only ids still in the layout. */
export function visiblePressed(
  held: ReadonlySet<string>,
  pending: ReadonlySet<string>,
  layout: Layout | null,
  status: ConnStatus,
): Set<string> {
  if (status !== "open" || !layout) return new Set();
  const ids = new Set(layout.pages.flatMap((pg) => pg.buttons.map((b) => b.id)));
  return new Set([...held, ...pending].filter((id) => ids.has(id)));
}

/** Volume and media keys are tapped in quick runs (louder, skip, skip), so every tap counts. */
export const repeatable = (type: ActionType): boolean => type === "volume" || type === "media";

/** Repeatable keys stay unguarded; every other key waits for its pending press. */
export const canFire = (type: ActionType, id: string, pending: ReadonlySet<string>): boolean =>
  repeatable(type) || !pending.has(id);

/** "connecting" is only for the first connection; after that a drop reads "lost" until open again. */
export function displayStatus(raw: ConnStatus, everOpen: boolean): ConnStatus {
  if (raw === "open" || raw === "kicked") return raw;
  return everOpen ? "closed" : "connecting";
}
