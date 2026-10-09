/** Authoritative native readback distinguishes complete engine support from similarly named methods. */
import { POLICY_VERSION } from './constants.ts';
import type { NativePolicy, PolicyEngine, PolicyState } from './types.ts';

/** Check the exact readback contract before trusting state supplied by the engine seam. */
function stateShape(value: Partial<PolicyState>): boolean {
  return (
    value.version === POLICY_VERSION &&
    typeof value.rendererStarted === 'boolean' &&
    typeof value.timeZone === 'string' &&
    typeof value.locale === 'string' &&
    typeof value.hardwareConcurrency === 'number' &&
    typeof value.acceptLanguages === 'string'
  );
}
/** Missing or unknown contracts cannot silently enable a partly patched distribution. */
export function readState(engine: PolicyEngine): PolicyState {
  const state = engine._getOyaSessionPolicy();
  if (!state || typeof state !== 'object' || !stateShape(state)) throw new Error('Unsupported native policy readback');
  return state as PolicyState;
}
/** Match the full subset and require installation to finish before any renderer starts. */
function nativePolicyMatches(state: PolicyState, policy: NativePolicy): boolean {
  return (
    !state.rendererStarted &&
    state.timeZone === policy.timeZone &&
    state.locale === policy.locale &&
    state.hardwareConcurrency === policy.hardwareConcurrency &&
    state.acceptLanguages === policy.languages.join(',')
  );
}
/** Do not expose a session whose engine ignored, altered or incompletely applied requested protection. */
export function verifyInstalled(engine: PolicyEngine, policy: NativePolicy): void {
  if (!nativePolicyMatches(readState(engine), policy))
    throw new Error('Native session policy readback does not match the requested protection');
}
