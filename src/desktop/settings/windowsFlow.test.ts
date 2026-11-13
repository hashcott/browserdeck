import { expect, test } from "vitest";
import { windowsFlow, type WindowsFlowState } from "./windowsFlow";

function harness(setAdminMode: (on: boolean) => Promise<void>) {
  let state: WindowsFlowState = { confirmAdmin: false, busy: false, error: null };
  const log: string[] = [];
  const flow = windowsFlow({
    api: { setAdminMode: (on) => { log.push(`setAdminMode(${on})`); return setAdminMode(on); }, setAutostart: async () => {} },
    reload: async () => { log.push(`reload busy=${state.busy}`); },
    set: (patch) => { state = { ...state, ...patch }; },
    errorText: (code) => `text:${String(code)}`,
  });
  return { flow, log, state: () => state };
}

test("turning administrator mode on: confirm, the command fails, the error shows and the config is reloaded", async () => {
  const h = harness(() => Promise.reject("uac_declined"));
  h.flow.askAdmin();
  expect(h.state()).toEqual({ confirmAdmin: true, busy: false, error: null });
  expect(h.log).toEqual([]); // nothing runs before the confirmation
  const running = h.flow.setAdmin(true);
  expect(h.state()).toMatchObject({ confirmAdmin: false, busy: true });
  await running;
  expect(h.log).toEqual(["setAdminMode(true)", "reload busy=true"]);
  expect(h.state()).toEqual({ confirmAdmin: false, busy: false, error: "text:uac_declined" });
});

test("cancel runs nothing; a later success clears the old error", async () => {
  const h = harness(() => Promise.resolve());
  h.flow.askAdmin();
  h.flow.cancelAdmin();
  expect(h.state().confirmAdmin).toBe(false);
  expect(h.log).toEqual([]);
  h.flow.askAdmin();
  await h.flow.setAdmin(false);
  expect(h.state()).toEqual({ confirmAdmin: false, busy: false, error: null });
  expect(h.log).toEqual(["setAdminMode(false)", "reload busy=true"]);
});
