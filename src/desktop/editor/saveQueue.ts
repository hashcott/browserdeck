/** Serialises saves: one in flight, and of the ones that arrive meanwhile only the latest is sent next.
 *  A failure drops the queued save and reports the last value the core accepted. */
export function createSaveQueue<T>(
  send: (v: T) => Promise<void>,
  onSaved: (v: T) => void,
  onFailed: (err: unknown, lastGood: T | undefined) => void,
  initial?: T,
) {
  let good = initial;
  let next: { v: T } | null = null;
  let running: Promise<void> | null = null;

  async function pump() {
    while (next) {
      const { v } = next;
      next = null;
      try {
        await send(v);
        good = v;
        onSaved(v);
      } catch (err) {
        next = null;
        onFailed(err, good);
      }
    }
    running = null;
  }
  return {
    save(v: T) {
      next = { v };
      running ??= pump();
    },
    /** Resolves when nothing is in flight or queued. */
    idle: async () => { while (running) await running; },
  };
}
