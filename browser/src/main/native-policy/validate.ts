/** Validate the entire supported policy subset before calling any immutable engine setter. */
import { MAX_POLICY_TEXT, MIN_PROCESSORS, MAX_PROCESSORS, POLICY_FIELDS, NATIVE_PLATFORMS } from './constants.ts';
import { userAgentOf, metadataOf } from './metadata.ts';
import type { NativePolicy } from './types.ts';

/** Reject hidden unsupported fields instead of silently claiming full persona coverage. */
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Native policy must be an object');
  const keys = Reflect.ownKeys(value);
  if (keys.length !== POLICY_FIELDS.length || keys.some((key) => !POLICY_FIELDS.includes(key as never)))
    throw new Error('Native policy requires exactly the supported identity fields');
  return value as Record<string, unknown>;
}
/** Bound strings before ICU parsing and reject control characters or embedded NULs. */
function text(value: unknown): string {
  if (typeof value !== 'string' || !value || value.length > MAX_POLICY_TEXT || !/^[\x21-\x7e]+$/.test(value))
    throw new Error('Invalid native policy text');
  return value;
}
/** Canonicalize supported language tags without admitting language-less ICU defaults. */
function localeOf(value: unknown): string {
  const locale = Intl.getCanonicalLocales(text(value))[0];
  if (new Intl.Locale(locale).language === 'und') throw new Error('Native policy requires a language');
  return locale;
}
/** Numeric offset timezones are not the ICU system zones supported by this primitive. */
function nativePolicyZone(value: unknown): string {
  const zone = text(value);
  if (!/^[A-Za-z][A-Za-z0-9_+/-]*$/.test(zone)) throw new Error('Native policy requires a system timezone');
  return new Intl.DateTimeFormat('en-US', { timeZone: zone }).resolvedOptions().timeZone;
}
/** Reject coercion, fractions and counts the engine cannot represent. */
function processors(value: unknown): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < MIN_PROCESSORS || value > MAX_PROCESSORS)
    throw new Error('Invalid native processor count');
  return value;
}
/** Dense lists only: array holes must not bypass validation of a requested language. */
function matchingLanguages(value: unknown, expected: string[]): boolean {
  return (
    Array.isArray(value) &&
    value.length === expected.length &&
    Array.from(value).every((tag, i) => localeOf(tag) === expected[i])
  );
}
/** Keep the requested list coherent with the engine-owned locale and primary fallback. */
function languagesOf(value: unknown, locale: string): readonly string[] {
  const primary = new Intl.Locale(locale).language;
  const expected = locale === primary ? [locale] : [locale, primary];
  if (!matchingLanguages(value, expected))
    throw new Error('Native languages must match the locale and its primary fallback');
  return Object.freeze(expected);
}
/** Reject unsupported or coerced platform values before the first irreversible native mutation. */
function platformOf(value: unknown): NativePolicy['platform'] {
  if (!NATIVE_PLATFORMS.includes(value as never)) throw new Error('Invalid native platform');
  return value as NativePolicy['platform'];
}
/** Return an immutable snapshot; validation alone does not mutate or certify an engine session. */
export function validatePolicy(value: unknown): NativePolicy {
  const input = record(value);
  const locale = localeOf(input.locale);
  const timeZone = nativePolicyZone(input.timeZone);
  const hardwareConcurrency = processors(input.hardwareConcurrency);
  const languages = languagesOf(input.languages, locale);
  const platform = platformOf(input.platform);
  const identity = { userAgent: userAgentOf(input.userAgent), userAgentMetadata: metadataOf(input.userAgentMetadata) };
  return Object.freeze({ timeZone, locale, hardwareConcurrency, languages, platform, ...identity });
}
