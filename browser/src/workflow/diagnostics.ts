/**
 * Redaction for anything that leaves the machine or the worker: diagnostics
 * bundles, run events and error messages. Private keys, credentials in URLs,
 * bearer tokens and known secret values are removed.
 */
import { REDACT } from './constants.ts';

/** Keys whose values are never kept, whatever they hold. */
const PRIVATE =
  /password|passwd|secret|token|authorization|cookie|api.?key|otp|one.?time|fingerprint|origins|text|value|body|payload|params|headers/i;

/** A URL without credentials, query or fragment; anything unparsable is replaced whole. */
export function safeUrl(raw: string): string {
  try {
    const url = new URL(raw);
    Object.assign(url, { username: '', password: '', search: '', hash: '' });
    return url.toString();
  } catch {
    return '[redacted URL]';
  }
}

/** A string with URLs cleaned, bearer tokens and known secrets removed, and length capped. */
function redactText(input: string, secrets: readonly unknown[]): string {
  let text = input.replace(/https?:\/\/[^\s"<>]+/g, safeUrl).replace(/Bearer\s+[^\s"']+/gi, 'Bearer [redacted]');
  for (const value of secrets) {
    if (value && String(value).length > REDACT.MIN_SECRET_LENGTH) text = text.split(String(value)).join('[redacted]');
  }
  return text.slice(0, REDACT.MAX_TEXT);
}

/** An object with private keys blanked and every other value redacted in turn. */
function redactObject(input: object, secrets: readonly unknown[], depth: number): Record<string, unknown> {
  const entries = Object.entries(input).slice(0, REDACT.MAX_ITEMS);
  return Object.fromEntries(
    entries.map(([key, value]) => [key, PRIVATE.test(key) ? '[redacted]' : redact(value, secrets, depth + 1)]),
  );
}

/** A copy of `input` safe to show or export; `secrets` are values to strip wherever they appear. */
export function redact<T>(input: T, secrets?: readonly unknown[], depth?: number): T;
export function redact(input: unknown, secrets: readonly unknown[] = [], depth = 0): unknown {
  if (depth > REDACT.MAX_DEPTH) return '[omitted]';
  if (typeof input === 'string') return redactText(input, secrets);
  if (Array.isArray(input)) return input.slice(0, REDACT.MAX_ITEMS).map((value) => redact(value, secrets, depth + 1));
  if (input && typeof input === 'object') return redactObject(input, secrets, depth);
  return input;
}
