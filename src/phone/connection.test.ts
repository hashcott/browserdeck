import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createConnection, type ConnStatus } from "./connection";

class FakeWS {
  static all: FakeWS[] = [];
  static OPEN = 1;
  readyState = 0;
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  constructor(public url: string) { FakeWS.all.push(this); }
  send(s: string) { this.sent.push(s); }
  close() { this.readyState = 3; this.onclose?.(); }
  open() { this.readyState = 1; this.onopen?.(); }
  drop() { this.readyState = 3; this.onclose?.(); }
  recv(m: unknown) { this.onmessage?.({ data: JSON.stringify(m) }); }
}

let statuses: ConnStatus[];
let layouts: unknown[];
const make = () =>
  createConnection({
    url: "ws://h/ws",
    onLayout: (m) => layouts.push(m),
    onStatus: (s) => statuses.push(s),
    WebSocketImpl: FakeWS as unknown as typeof WebSocket,
  });
const last = () => FakeWS.all[FakeWS.all.length - 1];

beforeEach(() => { vi.useFakeTimers(); FakeWS.all = []; statuses = []; layouts = []; });
afterEach(() => vi.useRealTimers());

test("press resolves ok when the matching result arrives", async () => {
  const c = make(); last().open();
  const p = c.press("b");
  expect(JSON.parse(last().sent[0])).toEqual({ type: "press", press_id: 1, button_id: "b" });
  last().recv({ type: "result", press_id: 1, ok: true, error: null });
  expect(await p).toEqual({ ok: true });
});
test("press resolves with the server error code", async () => {
  const c = make(); last().open();
  const p = c.press("b");
  last().recv({ type: "result", press_id: 1, ok: false, error: "obs_not_connected" });
  expect(await p).toEqual({ ok: false, error: "obs_not_connected" });
});
test("results are matched by press_id, not order", async () => {
  const c = make(); last().open();
  const a = c.press("a"), b = c.press("b");
  last().recv({ type: "result", press_id: 2, ok: false, error: "launch_failed" });
  last().recv({ type: "result", press_id: 1, ok: true, error: null });
  expect(await a).toEqual({ ok: true });
  expect(await b).toEqual({ ok: false, error: "launch_failed" });
});
test("press times out after 5000 ms", async () => {
  const c = make(); last().open();
  const p = c.press("b");
  await vi.advanceTimersByTimeAsync(4999);
  let done = false; void p.then(() => (done = true));
  await Promise.resolve();
  expect(done).toBe(false);
  await vi.advanceTimersByTimeAsync(1);
  expect(await p).toEqual({ ok: false, error: "timeout" });
});
test("press while disconnected fails immediately with timeout", async () => {
  const c = make();
  expect(await c.press("b")).toEqual({ ok: false, error: "timeout" });
  expect(last().sent).toEqual([]);
});
test("reconnects with backoff 500, 1000, 2000 … capped at 5000", () => {
  make(); last().drop();
  const delays = [500, 1000, 2000, 4000, 5000, 5000];
  delays.forEach((d, i) => {
    expect(FakeWS.all).toHaveLength(i + 1);
    vi.advanceTimersByTime(d - 1);
    expect(FakeWS.all).toHaveLength(i + 1);
    vi.advanceTimersByTime(1);
    expect(FakeWS.all).toHaveLength(i + 2);
    last().drop();
  });
});
test("backoff returns to 500 ms after a successful connection", () => {
  make(); last().drop(); vi.advanceTimersByTime(500); last().drop(); vi.advanceTimersByTime(1000);
  last().open(); last().drop();
  vi.advanceTimersByTime(500);
  expect(FakeWS.all).toHaveLength(4);
});
test("does not reconnect after kicked", () => {
  make(); last().open();
  last().recv({ type: "kicked" });
  last().drop();
  vi.advanceTimersByTime(60000);
  expect(FakeWS.all).toHaveLength(1);
  expect(statuses[statuses.length - 1]).toBe("kicked");
});
test("layout messages reach onLayout", () => {
  make(); last().open();
  const m = { type: "layout", pages: [], grid: { cols: 3, rows: 4 }, language: "auto", theme: {} };
  last().recv(m);
  expect(layouts).toEqual([m]);
});
test("close() stops reconnecting", () => {
  const c = make(); last().open(); c.close();
  vi.advanceTimersByTime(60000);
  expect(FakeWS.all).toHaveLength(1);
});
