import { expect, test } from "vitest";
import { pairingView } from "./pairing";

const a = { ip: "192.168.1.5", url: "http://192.168.1.5:47825/?token=t" };
const b = { ip: "10.0.0.7", url: "http://10.0.0.7:47825/?token=t" };

test("no addresses means no_network, never a QR", () => {
  expect(pairingView({ addresses: [], selected: null })).toEqual({ kind: "no_network" });
  expect(pairingView({ addresses: [], selected: "192.168.1.5" })).toEqual({ kind: "no_network" });
});
test("uses the selected address", () => {
  expect(pairingView({ addresses: [a, b], selected: "10.0.0.7" })).toEqual({ kind: "ready", url: b.url, ip: b.ip, others: [a.ip] });
});
test("falls back to the first address when selected is missing from the list", () => {
  expect(pairingView({ addresses: [a, b], selected: "172.16.0.1" })).toEqual({ kind: "ready", url: a.url, ip: a.ip, others: [b.ip] });
  expect(pairingView({ addresses: [a, b], selected: null })).toEqual({ kind: "ready", url: a.url, ip: a.ip, others: [b.ip] });
});
test("one address has no others", () => {
  expect(pairingView({ addresses: [a], selected: a.ip })).toEqual({ kind: "ready", url: a.url, ip: a.ip, others: [] });
});
