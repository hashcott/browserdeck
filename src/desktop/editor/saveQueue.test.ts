import { expect, test } from "vitest";
import { createSaveQueue } from "./saveQueue";

const defer = () => {
  let resolve!: () => void, reject!: (e: unknown) => void;
  const p = new Promise<void>((a, b) => { resolve = a; reject = b; });
  return { p, resolve, reject };
};

test("at most one save in flight; only the latest pending one is sent next", async () => {
  const gates = [defer(), defer()];
  const sent: number[] = [];
  const q = createSaveQueue<number>((v) => { sent.push(v); return gates[sent.length - 1].p; }, () => {}, () => {});
  q.save(1); q.save(2); q.save(3);
  expect(sent).toEqual([1]);
  gates[0].resolve();
  await Promise.resolve(); await Promise.resolve();
  expect(sent).toEqual([1, 3]);
  gates[1].resolve();
  await q.idle();
});
test("a failure reports the last good value and drops the pending save", async () => {
  const gates = [defer(), defer()];
  let n = 0;
  const failed: [unknown, number | undefined][] = [];
  const q = createSaveQueue<number>(() => gates[n++].p, () => {}, (e, good) => failed.push([e, good]), 0);
  q.save(1); q.save(2);
  gates[0].reject("save_failed");
  await q.idle();
  expect(failed).toEqual([["save_failed", 0]]);
  expect(n).toBe(1);
});
test("a success updates the last good value", async () => {
  const good: number[] = [];
  const q = createSaveQueue<number>(() => Promise.resolve(), (v) => good.push(v), () => {});
  q.save(1); q.save(2);
  await q.idle();
  expect(good).toEqual([1, 2]);
});
