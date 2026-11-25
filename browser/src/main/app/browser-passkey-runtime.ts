/** Refuse the experimental phone path on an unchanged runtime instead of silently ignoring an invented API. */
import { BrowserPasskeys } from './browser-passkeys.ts';
import { BROWSER_PASSKEY_SWITCH, BROWSER_PASSKEY_PROTOCOL_VERSION } from './constants.ts';
import type { BrowserPasskeyDeps } from './browser-passkey-types.ts';

/** This method is implemented by the exact native patch and exposes no credential/request material. */
interface BrowserPasskeyRuntime {
  /** Reports native protocol support and whether the process meets native platform/entitlement gates. */
  getOyaBrowserPasskeyCapabilities?: () => {
    /** Both native policy phases are implemented. */ protocolVersion: number;
    /** Explicit opt-in and platform authorization succeeded. */ enabled: boolean;
  };
}

/** Production stock launches create no new strategy or native event listeners. */
export function createBrowserPasskeys(
  deps: BrowserPasskeyDeps,
  args = process.argv,
  platform = process.platform,
): BrowserPasskeys | undefined {
  if (!args.includes(BROWSER_PASSKEY_SWITCH)) return;
  requireRuntime(deps, platform);
  return new BrowserPasskeys(deps);
}
/** The exact native protocol must be active before any private policy listeners are installed. */
function requireRuntime(deps: BrowserPasskeyDeps, platform: string): void {
  if (!available(deps, platform))
    throw Error(
      'This experimental runtime cannot provide native browser passkeys. Use a verified matching test build.',
    );
}
/** A declared flag alone cannot enable a stock runtime or an unprovisioned native process. */
function available(deps: BrowserPasskeyDeps, platform: string): boolean {
  try {
    return nativeCapability(deps, platform);
  } catch {
    return false;
  }
}
/** Runtime exceptions are never promoted into sensitive startup diagnostics. */
function nativeCapability(deps: BrowserPasskeyDeps, platform: string): boolean {
  const runtime = deps.electron.app as unknown as BrowserPasskeyRuntime;
  const capability = runtime.getOyaBrowserPasskeyCapabilities?.();
  return (
    platform === 'darwin' &&
    capability?.protocolVersion === BROWSER_PASSKEY_PROTOCOL_VERSION &&
    capability.enabled === true
  );
}
