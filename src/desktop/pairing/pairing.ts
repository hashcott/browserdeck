import type { PairingInfo } from "../../shared/types";

export type PairingView = { kind: "no_network" } | { kind: "ready"; url: string; ip: string; others: string[] };

/** No address, no QR. The selected address wins; one that is not on the list falls back to the first. */
export function pairingView(info: PairingInfo): PairingView {
  const cur = info.addresses.find((a) => a.ip === info.selected) ?? info.addresses[0];
  if (!cur) return { kind: "no_network" };
  return { kind: "ready", url: cur.url, ip: cur.ip, others: info.addresses.filter((a) => a !== cur).map((a) => a.ip) };
}
