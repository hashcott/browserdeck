/** The Windows section's state: the administrator confirmation, a running command, its error. */
export interface WindowsFlowState { confirmAdmin: boolean; busy: boolean; error: string | null }

export interface WindowsFlowDeps {
  api: { setAdminMode(enabled: boolean): Promise<void>; setAutostart(enabled: boolean): Promise<void> };
  /** Takes the core's config and runtime as they are now. */
  reload(): Promise<unknown>;
  set(patch: Partial<WindowsFlowState>): void;
  errorText(code: unknown): string;
}

/** Administrator mode is only turned on from its confirmation. Both settings are saved by their own commands
 *  (desktop saves never change them), so after each command, failed or not, the config and runtime are reloaded. */
export function windowsFlow(d: WindowsFlowDeps) {
  const run = async (call: () => Promise<void>) => {
    d.set({ busy: true, error: null });
    let error: string | null = null;
    try { await call(); } catch (e) { error = d.errorText(e); }
    await d.reload().catch(() => {});
    d.set({ busy: false, error });
  };
  return {
    askAdmin: () => d.set({ confirmAdmin: true, error: null }),
    cancelAdmin: () => d.set({ confirmAdmin: false }),
    setAdmin: (enabled: boolean) => { d.set({ confirmAdmin: false }); return run(() => d.api.setAdminMode(enabled)); },
    setAutostart: (enabled: boolean) => run(() => d.api.setAutostart(enabled)),
  };
}
