/** Named native failures preserve actual network completion semantics without an inspector backend. */
import type { BodySession } from './types.ts';
/** Every name has an explicit native net-error mapping in the patched engine. */
const REASONS = new Set([
  'Failed',
  'Aborted',
  'TimedOut',
  'AccessDenied',
  'ConnectionClosed',
  'ConnectionReset',
  'ConnectionRefused',
  'ConnectionAborted',
  'ConnectionFailed',
  'NameNotResolved',
  'InternetDisconnected',
  'AddressUnreachable',
  'BlockedByClient',
  'BlockedByResponse',
]);
/** Never accept arbitrary strings or native error numbers from an external caller. */
export function validateFailureReason(value: unknown): asserts value is string {
  if (typeof value !== 'string' || !REASONS.has(value)) throw Error('Unsupported native request failure reason');
}
/** Old engines may only perform their real default cancellation, never silently substitute another reason. */
export function requireFailureEngine(session: BodySession, cancel: boolean, reason?: string): void {
  if (reason === undefined) return;
  validateFailureReason(reason);
  if (!cancel) throw Error('A continued request cannot specify a failure reason');
  if (reason !== 'BlockedByClient' && session.webRequest._supportsOyaRequestErrors?.() !== true)
    throw Error('This Oya engine does not support native request failure reasons');
}
