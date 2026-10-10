/** Preflight native identity bounds and copy every nested field before irreversible session mutation. */
import { FORM_FACTORS, MAX_BRANDS, MAX_POLICY_TEXT, MAX_USER_AGENT, METADATA_FIELDS } from './constants.ts';
import type { NativeBrand, NativeMetadata } from './metadata-types.ts';

/** Exact own-key records reject hidden fields rather than certifying ignored values. */
function metadataRecord(value: unknown, fields: readonly string[]): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid native metadata object');
  const keys = Reflect.ownKeys(value);
  if (keys.length !== fields.length || keys.some((key) => !fields.includes(key as string)))
    throw Error('Invalid native metadata fields');
  return value as Record<string, unknown>;
}
/** Match native printable ASCII limits without coercion or stripping whitespace. */
function identityText(value: unknown, required = false, maximum = MAX_POLICY_TEXT): string {
  if (typeof value !== 'string' || value.length > maximum || (required && !value) || !/^[\x20-\x7e]*$/.test(value))
    throw Error('Invalid native identity text');
  return value;
}
/** User-Agent strings have their own larger bound and cannot be empty. */
export function userAgentOf(value: unknown): string {
  return identityText(value, true, MAX_USER_AGENT);
}
/** Boolean metadata must never acquire truthiness-based meaning. */
function boolean(value: unknown): boolean {
  if (typeof value !== 'boolean') throw Error('Invalid native metadata boolean');
  return value;
}
/** Dense bounded arrays prevent sparse entries from bypassing validation. */
function list(value: unknown, maximum: number): unknown[] {
  if (!Array.isArray(value) || !value.length || value.length > maximum) throw Error('Invalid native metadata list');
  return Array.from(value);
}
/** Freeze a fresh pair; no caller-owned brand objects enter the installed policy. */
function brandOf(value: unknown): NativeBrand {
  const pair = metadataRecord(value, ['brand', 'version']);
  return Object.freeze({ brand: identityText(pair.brand, true), version: identityText(pair.version, true) });
}
/** Ordered brand names must be unique within each native list. */
function brandsOf(value: unknown): readonly NativeBrand[] {
  const brands = list(value, MAX_BRANDS).map(brandOf);
  if (new Set(brands.map((pair) => pair.brand)).size !== brands.length) throw Error('Duplicate native metadata brand');
  return Object.freeze(brands);
}
/** Native low/high lists must describe the same names in the same order. */
function versionsOf(value: unknown, brands: readonly NativeBrand[]): readonly NativeBrand[] {
  const versions = brandsOf(value);
  if (versions.length !== brands.length || versions.some((pair, i) => pair.brand !== brands[i].brand))
    throw Error('Native metadata brand lists disagree');
  return versions;
}
/** Reject unknown or duplicated form factors rather than silently filtering them. */
function factorsOf(value: unknown): readonly string[] {
  const factors = list(value, FORM_FACTORS.length);
  if (factors.some((factor) => !FORM_FACTORS.includes(factor as never)) || new Set(factors).size !== factors.length)
    throw Error('Invalid native metadata form factors');
  return Object.freeze(factors as string[]);
}
/** Normalize descriptive strings in a fixed order for immutable reuse and native readback comparison. */
function stringsOf(input: Record<string, unknown>) {
  return {
    fullVersion: identityText(input.fullVersion, true),
    platform: identityText(input.platform, true),
    platformVersion: identityText(input.platformVersion),
    architecture: identityText(input.architecture),
    model: identityText(input.model),
    bitness: identityText(input.bitness),
  };
}
/** Validate all metadata before any setter; this is a shape contract, not full persona coherence. */
export function metadataOf(value: unknown): NativeMetadata {
  const input = metadataRecord(value, METADATA_FIELDS);
  const brands = brandsOf(input.brands);
  const fullVersionList = versionsOf(input.fullVersionList, brands);
  const strings = stringsOf(input);
  const mobile = boolean(input.mobile),
    wow64 = boolean(input.wow64);
  const formFactors = factorsOf(input.formFactors);
  return Object.freeze({ brands, fullVersionList, ...strings, mobile, wow64, formFactors });
}
