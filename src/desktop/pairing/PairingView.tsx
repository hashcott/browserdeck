import { QRCodeSVG } from "qrcode.react";
import { useId } from "react";
import type { createT } from "../../shared/i18n";
import type { PairingInfo } from "../../shared/types";
import { Icon } from "../editor/Icons";
import { CloseButton, Screen } from "../Screen";
import { pairingView } from "./pairing";
import "../screens.css";

type T = ReturnType<typeof createT>;
export type PairingState = { kind: "loading" } | { kind: "server_error"; text: string } | { kind: "load_error"; text: string } | { kind: "info"; info: PairingInfo };

export interface PairingViewProps {
  state: PairingState;
  /** A phone is connected right now. */
  phoneConnected: boolean;
  confirmingRegenerate: boolean;
  /** Set while a rejected save or a failed new code needs saying. */
  error: string | null;
  t: T;
  onPickIp(ip: string): void;
  onAskRegenerate(): void;
  onConfirmRegenerate(): void;
  onCancelRegenerate(): void;
  onRetry(): void;
  onBack(): void;
}

export function PairingScreen(p: PairingViewProps) {
  const { t } = p;
  const id = useId();
  const view = p.state.kind === "info" ? pairingView(p.state.info) : null;
  return (
    <Screen className="pairing-screen" onClose={p.onBack}>
      <header className="head">
        <Icon name="phone" size={20} />
        <h1>{t("pairing.title")}</h1>
        <CloseButton label={t("settings.back")} onClose={p.onBack} />
      </header>
      <main className="page">
        <div className="page-inner pairing">
          {p.phoneConnected && (
            <section className="banner ok" role="status" aria-labelledby={`${id}c`}>
              <Icon name="check" size={26} />
              <div>
                <h2 id={`${id}c`}>{t("pairing.connected")}</h2>
                <p>{t("pairing.connectedHint")}</p>
              </div>
              <button type="button" className="btn cta" onClick={p.onBack}>{t("pairing.toEditor")}</button>
            </section>
          )}

          {p.state.kind === "loading" && <p className="lead" role="status">{t("pairing.loading")}</p>}

          {p.state.kind === "server_error" && (
            <section className="banner bad" role="alert">
              <Icon name="alert" size={26} />
              <p>{p.state.text}</p>
            </section>
          )}

          {p.state.kind === "load_error" && (
            <section className="banner bad" role="alert">
              <Icon name="alert" size={26} />
              <p>{p.state.text}</p>
              <button type="button" className="btn cta" onClick={p.onRetry}>{t("common.retry")}</button>
            </section>
          )}

          {view?.kind === "no_network" && (
            <section className="banner bad" role="status">
              <Icon name="alert" size={26} />
              <p>{t("pairing.noNetwork")}</p>
              <button type="button" className="btn cta" onClick={p.onRetry}>{t("common.retry")}</button>
            </section>
          )}

          {view?.kind === "ready" && (
            <div className="pair-cols">
              <div className="qr-wrap">
                {/* The code encodes the full URL including the token. */}
                <div className="qr"><QRCodeSVG value={view.url} size={264} level="M" marginSize={2} title={t("pairing.scan")} /></div>
              </div>
              <div className="pair-side">
                <p className="lead">{t("pairing.intro")}</p>

                {view.others.length > 0 && (
                  <div className="fld">
                    <label htmlFor={`${id}ip`}>{t("pairing.address")}</label>
                    <select id={`${id}ip`} className="in" value={view.ip} onChange={(e) => p.onPickIp(e.target.value)} aria-describedby={`${id}iph`}>
                      {[view.ip, ...view.others].sort().map((ip) => <option key={ip} value={ip}>{ip}</option>)}
                    </select>
                    <p className="note" id={`${id}iph`}>{t("pairing.addressHint")}</p>
                  </div>
                )}

                <div className="fld">
                  <label className="label" htmlFor={`${id}u`}>{t("pairing.link")}</label>
                  {/* Read-only but selectable, and announced by assistive technology. */}
                  <textarea id={`${id}u`} className="url" readOnly rows={1} value={view.url} onFocus={(e) => e.currentTarget.select()} />
                  <p className="warn"><Icon name="alert" size={18} /><span>{t("pairing.publicWifi")} {t("pairing.secret")}</span></p>
                </div>

                <div className="notes">
                  <p className="note">{t("pairing.firewall")}</p>
                  <p className="note">{t("pairing.ipChanged")}</p>
                </div>

                <div className="regen">
                  {p.confirmingRegenerate ? (
                    <div role="group" aria-label={t("pairing.regenerate")} className="confirm-box">
                      <p>{t("pairing.regenerateConfirm")}</p>
                      <div className="row">
                        <button type="button" className="btn danger" onClick={p.onConfirmRegenerate}>{t("pairing.regenerateYes")}</button>
                        <button type="button" className="btn" autoFocus onClick={p.onCancelRegenerate}>{t("common.cancel")}</button>
                      </div>
                    </div>
                  ) : (
                    <button type="button" className="btn" onClick={p.onAskRegenerate}>{t("pairing.regenerate")}</button>
                  )}
                </div>
              </div>
            </div>
          )}

          {p.error && <p className="inline-error" role="alert"><Icon name="alert" size={18} />{p.error}</p>}
        </div>
      </main>
    </Screen>
  );
}
