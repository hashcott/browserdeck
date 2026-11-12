import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Config } from "../shared/types";
import { createConfigCore } from "./configCore";

const cfg = (language: string, port = 1): Config => ({
  schema_version: 1, pages: [],
  settings: { token: "", port, obs: { host: "h", port: 1, password: "" }, grid: { cols: 4, rows: 3 }, language, run_as_admin: false,
    autostart: false, tray_notice_shown: false, preferred_ip: null, theme: "keycap" },
});
const defer = () => {
  let resolve!: () => void, reject!: (e: unknown) => void;
  const p = new Promise<void>((a, b) => { resolve = a; reject = b; });
  return { p, resolve, reject };
};
const setup = (send: (c: Config) => Promise<void>) => {
  const shown: Config[] = [], errors: unknown[] = [];
  const core = createConfigCore(send, { onConfig: (c) => shown.push(c), onSaveError: (e) => errors.push(e) }, 300);
  core.replace(cfg("auto"));
  return { core, shown, errors };
};
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

test("a rejected save rolls every view back to the last config the core accepted", async () => {
  const sent: string[] = [];
  const { core, shown, errors } = setup(async (c) => { sent.push(c.settings.language); if (c.settings.language === "bad") throw "bad_grid"; });
  core.commit(cfg("vi")); await core.flush();
  core.commit(cfg("bad")); await core.flush();
  expect(errors).toEqual(["bad_grid"]);
  expect(core.latest()?.settings.language).toBe("vi");
  expect(shown[shown.length - 1]?.settings.language).toBe("vi");
});
test("an edit made while a save is in flight is sent next, and is dropped with a rejection of the first", async () => {
  const gates = [defer(), defer()];
  const sent: string[] = [];
  const { core, errors } = setup((c) => { sent.push(c.settings.language); return gates[sent.length - 1].p; });
  core.commit(cfg("en"));
  core.commit(cfg("vi")); // while "en" is in flight
  expect(sent).toEqual(["en"]);
  gates[0].reject("save_failed");
  await core.flush();
  expect(sent).toEqual(["en"]); // the queued edit was dropped
  expect(errors).toEqual(["save_failed"]);
  expect(core.latest()?.settings.language).toBe("auto");
});
test("an edit made while a save is in flight is sent after it succeeds", async () => {
  const gates = [defer(), defer()];
  const sent: string[] = [];
  const { core } = setup((c) => { sent.push(c.settings.language); return gates[sent.length - 1].p; });
  core.commit(cfg("en")); core.commit(cfg("vi")); core.commit(cfg("auto", 2));
  gates[0].resolve();
  await vi.advanceTimersByTimeAsync(0);
  expect(sent).toEqual(["en", "auto"]); // only the latest pending one
  gates[1].resolve();
  await core.flush();
});
test("slow edits are debounced; flush sends the pending one now", async () => {
  const sent: number[] = [];
  const { core } = setup(async (c) => { sent.push(c.settings.port); });
  core.commit(cfg("auto", 10), true);
  core.commit(cfg("auto", 11), true);
  await vi.advanceTimersByTimeAsync(299);
  expect(sent).toEqual([]);
  await vi.advanceTimersByTimeAsync(1);
  expect(sent).toEqual([11]);
  core.commit(cfg("auto", 12), true);
  await core.flush();
  expect(sent).toEqual([11, 12]);
  await vi.advanceTimersByTimeAsync(1000);
  expect(sent).toEqual([11, 12]); // the cancelled timer does not send again
});
test("replace adopts what the core holds and makes it the rollback point", async () => {
  const { core, shown, errors } = setup(async (c) => { if (c.settings.port === 99) throw "bad_port"; });
  core.commit(cfg("auto", 5), true);
  core.replace(cfg("vi", 7));
  await vi.advanceTimersByTimeAsync(1000); // the pending debounced edit was discarded, nothing sent
  core.commit(cfg("vi", 99)); await core.flush();
  expect(errors).toEqual(["bad_port"]);
  expect(shown[shown.length - 1]).toEqual(cfg("vi", 7));
});
