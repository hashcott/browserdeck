import type { ActionError, ClientMsg, ServerMsg, Usage } from "../shared/types";

export type ConnStatus = "connecting" | "open" | "closed" | "kicked";
export type PressOutcome = { ok: true } | { ok: false; error: ActionError | "timeout" };

const PRESS_TIMEOUT_MS = 5000;
const BACKOFF_START_MS = 500;
const BACKOFF_MAX_MS = 5000;

type Timers = { setTimeout: (f: () => void, ms: number) => unknown; clearTimeout: (id: unknown) => void };
// Looked up on each call so fake timers installed after import are honoured.
const globalTimers: Timers = {
  setTimeout: (f, ms) => setTimeout(f, ms),
  clearTimeout: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

export function createConnection(opts: {
  url: string;
  onLayout: (m: Extract<ServerMsg, { type: "layout" }>) => void;
  onUsage?: (m: Usage) => void;
  /** The app now serves a newer phone page than this one: reload. */
  onNewBuild?: () => void;
  onStatus: (s: ConnStatus) => void;
  WebSocketImpl?: typeof WebSocket; // injected in tests
  timers?: Timers; // injected in tests
}): { press(buttonId: string): Promise<PressOutcome>; close(): void } {
  const WS = opts.WebSocketImpl ?? WebSocket;
  const timers = opts.timers ?? globalTimers;
  const pending = new Map<number, (o: PressOutcome) => void>();
  let ws: WebSocket | null = null;
  let nextId = 1; // no crypto.randomUUID: it needs a secure context and the page is plain HTTP
  let delay = BACKOFF_START_MS;
  let retry: unknown;
  let stopped = false;
  let build: string | null = null; // this page's build: the first one the app reports

  const failAll = () => {
    for (const resolve of [...pending.values()]) resolve({ ok: false, error: "timeout" });
  };

  function connect() {
    opts.onStatus("connecting");
    const s = new WS(opts.url);
    ws = s;
    s.onopen = () => {
      delay = BACKOFF_START_MS;
      opts.onStatus("open");
    };
    s.onmessage = (e) => {
      let m: ServerMsg;
      try { m = JSON.parse(e.data as string) as ServerMsg; } catch { return; }
      if (m.type === "layout") {
        if (build !== null && m.build !== build) {
          stopped = true;
          opts.onNewBuild?.();
          return;
        }
        build = m.build;
        opts.onLayout(m);
      }
      else if (m.type === "usage") opts.onUsage?.(m);
      else if (m.type === "result") pending.get(m.press_id)?.(m.ok ? { ok: true } : { ok: false, error: m.error ?? "timeout" });
      else if (m.type === "kicked") {
        stopped = true;
        opts.onStatus("kicked");
      }
    };
    s.onclose = () => {
      if (ws !== s) return;
      ws = null;
      failAll();
      if (stopped) return;
      opts.onStatus("closed");
      retry = timers.setTimeout(connect, delay);
      delay = Math.min(delay * 2, BACKOFF_MAX_MS);
    };
  }
  connect();

  return {
    press(buttonId) {
      if (!ws || ws.readyState !== 1) return Promise.resolve({ ok: false, error: "timeout" });
      const press_id = nextId++;
      const socket = ws;
      return new Promise<PressOutcome>((resolve) => {
        const finish = (o: PressOutcome) => {
          timers.clearTimeout(timer);
          pending.delete(press_id);
          resolve(o);
        };
        const timer = timers.setTimeout(() => finish({ ok: false, error: "timeout" }), PRESS_TIMEOUT_MS);
        pending.set(press_id, finish);
        const msg: ClientMsg = { type: "press", press_id, button_id: buttonId };
        socket.send(JSON.stringify(msg));
      });
    },
    close() {
      stopped = true;
      timers.clearTimeout(retry);
      const s = ws;
      ws = null;
      failAll();
      s?.close();
    },
  };
}
