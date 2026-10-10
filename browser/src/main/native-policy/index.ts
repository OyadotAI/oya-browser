/** Public seam for the native policy subset; deliberately not a full-persona protection adapter. */
export { NativeSessionPolicies } from './install.ts';
export type { NativePolicy, PolicySession } from './types.ts';
export { nativePolicyForPersona, nativePolicyForHost } from './persona.ts';
export { NativeSessionProtection } from './protection.ts';
export type { PreScriptPolicy, ProtectedSession } from './protection.ts';
