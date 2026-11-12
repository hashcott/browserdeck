import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import { createT } from "../../shared/i18n";

const qr = vi.hoisted(() => ({ values: [] as string[] }));
vi.mock("qrcode.react", () => ({ QRCodeSVG: (p: { value: string }) => { qr.values.push(p.value); return <svg data-qr /> ; } }));
import { PairingScreen, type PairingState } from "./PairingView";

const t = createT("en");
const url = "http://192.168.1.5:47825/?token=abc";
const render = (state: PairingState, extra = {}) => renderToStaticMarkup(
  <PairingScreen state={state} phoneConnected={false} confirmingRegenerate={false} error={null} t={t}
    onPickIp={() => {}} onAskRegenerate={() => {}} onConfirmRegenerate={() => {}} onCancelRegenerate={() => {}} onRetry={() => {}} onBack={() => {}} {...extra} />);
const ready: PairingState = { kind: "info", info: { addresses: [{ ip: "192.168.1.5", url }], selected: "192.168.1.5" } };

test("the QR encodes the full URL including the token, and the URL is shown with the warnings", () => {
  qr.values.length = 0;
  const html = render(ready);
  expect(qr.values).toEqual([url]);
  expect(html).toContain(url.replace(/&/g, "&amp;"));
  expect(html).toContain(t("pairing.publicWifi"));
  expect(html).toContain(t("pairing.firewall"));
  expect(html).toContain(t("pairing.ipChanged"));
  expect(html).not.toContain("<select");
});
test("several addresses give a select", () => {
  const info = { addresses: [{ ip: "192.168.1.5", url }, { ip: "10.0.0.2", url: "http://10.0.0.2:47825/?token=abc" }], selected: null };
  expect(render({ kind: "info", info })).toContain("<select");
});
test("no network: text and retry, never a QR", () => {
  qr.values.length = 0;
  const html = render({ kind: "info", info: { addresses: [], selected: null } });
  expect(qr.values).toEqual([]);
  expect(html).toContain(t("pairing.noNetwork"));
  expect(html).toContain(t("common.retry"));
});
test("server error: explanation, no QR", () => {
  qr.values.length = 0;
  const html = render({ kind: "server_error", text: "The phone server could not start" });
  expect(qr.values).toEqual([]);
  expect(html).toContain("The phone server could not start");
});
test("a connected phone shows the confirmation and a way on", () => {
  const html = render(ready, { phoneConnected: true });
  expect(html).toContain(t("pairing.connected"));
  expect(html).toContain(t("pairing.toEditor"));
});
test("regenerate asks first and says the phone is disconnected", () => {
  const html = render(ready, { confirmingRegenerate: true });
  expect(html).toContain(t("pairing.regenerateConfirm"));
});
