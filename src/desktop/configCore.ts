import type { Config } from "../shared/types";
import { createSaveQueue } from "./editor/saveQueue";

export const SLOW_MS = 300;

export interface CoreHooks {
  /** The config the UI should show changed (an edit, or a rollback). */
  onConfig(c: Config): void;
  /** A save was rejected; the config has already been rolled back to the last one the core accepted. */
  onSaveError(err: unknown): void;
}

/** The non-React heart of the config store: one serialised save path, rollback on rejection, debounced edits. */
export function createConfigCore(send: (c: Config) => Promise<void>, hooks: CoreHooks, slowMs = SLOW_MS) {
  let cur: Config | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let queue: ReturnType<typeof createSaveQueue<Config>> | undefined;

  const makeQueue = (initial: Config) => createSaveQueue<Config>(send, () => {}, (err, good) => {
    if (good) { cur = good; hooks.onConfig(good); }
    hooks.onSaveError(err);
  }, initial);
  const cancelTimer = () => { clearTimeout(timer); timer = undefined; };

  const core = {
    /** Adopts a config the core (Rust) holds: first load, or a reload after the core changed it itself. */
    replace(c: Config) {
      cancelTimer();
      cur = c;
      queue = makeQueue(c);
      hooks.onConfig(c);
    },
    /** `slow` = typing or dragging: the save is debounced and a newer edit replaces a pending one. */
    commit(next: Config, slow = false) {
      cur = next;
      hooks.onConfig(next);
      cancelTimer();
      if (slow) timer = setTimeout(() => { timer = undefined; queue!.save(cur!); }, slowMs);
      else queue!.save(next);
    },
    /** Sends a pending debounced edit now and waits until nothing is in flight. */
    async flush() {
      if (timer) { cancelTimer(); queue?.save(cur!); }
      await queue?.idle();
    },
    /** The config as it is right now, including edits React has not rendered yet. */
    latest: () => cur,
  };
  return core;
}
