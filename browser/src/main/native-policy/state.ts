/** Authoritative native readback distinguishes complete engine support from similarly named methods. */
import { metadataOf, userAgentOf } from './metadata.ts';
import { POLICY_VERSION, NATIVE_PLATFORMS } from './constants.ts';
import type { NativePolicy, PolicyEngine, PolicyState } from './types.ts';

/** An unconfigured platform is absent, never an empty or arbitrary string. */
function platformShape(value: Partial<PolicyState>): boolean {
  return !Object.hasOwn(value, 'platform') || NATIVE_PLATFORMS.includes(value.platform as never);
}
/** Validate optional pre-install metadata and the required UA contract before mutation. */
function identityShape(value: Partial<PolicyState>): boolean {
  if (value.userAgent !== '') userAgentOf(value.userAgent);
  if (Object.hasOwn(value, 'userAgentMetadata')) metadataOf(value.userAgentMetadata);
  return true;
}
/** Compare normalized metadata rather than relying on native object property order. */
function identityMatches(state: PolicyState, policy: NativePolicy): boolean {
  return (
    state.userAgent === policy.userAgent &&
    JSON.stringify(metadataOf(state.userAgentMetadata)) === JSON.stringify(policy.userAgentMetadata)
  );
}
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
  if (!state || typeof state !== 'object' || !stateShape(state) || !platformShape(state) || !identityShape(state))
    throw new Error('Unsupported native policy readback');
  return state as PolicyState;
}
/** Match the full subset and require installation to finish before any renderer starts. */
function nativePolicyMatches(state: PolicyState, policy: NativePolicy): boolean {
  return (
    !state.rendererStarted &&
    state.platform === policy.platform &&
    state.timeZone === policy.timeZone &&
    state.locale === policy.locale &&
    state.hardwareConcurrency === policy.hardwareConcurrency &&
    state.acceptLanguages === policy.languages.join(',')
  );
}
/** Do not expose a session whose engine ignored, altered or incompletely applied requested protection. */
export function verifyInstalled(engine: PolicyEngine, policy: NativePolicy): void {
  const state = readState(engine);
  if (!nativePolicyMatches(state, policy) || !identityMatches(state, policy))
    throw new Error('Native session policy readback does not match the requested protection');
}
