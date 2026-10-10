/** Explicit policy subset and browser-owned engine seam; never a full-persona protection verdict. */
import type { NativeMetadata } from './metadata-types.ts';
import type { NATIVE_PLATFORMS } from './constants.ts';

/** Immutable settings that the currently implemented native primitives can enforce. */
export interface NativePolicy {
  /** Immutable native UA string shared by network, pages and workers. */
  readonly userAgent: string;
  /** Deep-frozen native low/high entropy metadata. */
  readonly userAgentMetadata: NativeMetadata;
  /** Web-exposed legacy desktop string, not the host operating system. */
  readonly platform: (typeof NATIVE_PLATFORMS)[number];
  /** Canonical ICU system timezone, not a numeric UTC offset. */
  readonly timeZone: string;
  /** Canonical BCP47 locale used by native ICU and language preferences. */
  readonly locale: string;
  /** Web-exposed logical processor count, not an OS scheduling quota. */
  readonly hardwareConcurrency: number;
  /** The locale followed by its primary-language fallback, when different. */
  readonly languages: readonly string[];
}
/** Optional capabilities allow explicit rejection of older engines before mutation. */
export interface PolicySession {
  /** Install the UA before metadata and before any renderer starts. */
  _setOyaUserAgent?: (value: string) => void;
  /** Install native UA metadata without request interception or page shims. */
  _setOyaUserAgentMetadata?: (value: NativeMetadata) => void;
  /** Install the legacy platform before any document or worker renderer starts. */
  _setOyaPlatform?: (platform: string) => void;
  /** Read authoritative native values and the first-renderer lock. */
  _getOyaSessionPolicy?: () => unknown;
  /** Install the timezone before the first renderer. */
  _setOyaTimeZone?: (zone: string) => void;
  /** Install the web-exposed native logical processor count. */
  _setOyaHardwareConcurrency?: (count: number) => void;
  /** Install native ICU, navigator language and default request-header policy. */
  _setOyaLocale?: (locale: string) => void;
}
/** The complete native seam after capability preflight. */
export type PolicyEngine = Required<PolicySession>;
/** One owner's permanent record of an attempted immutable session configuration. */
export interface PolicyBinding {
  /** The private normalized snapshot, never the caller's mutable object. */
  policy: NativePolicy;
  /** Failed or reentrant installation must not be mistaken for completed protection. */
  state: 'installing' | 'installed' | 'failed';
}

/** Native readback used to reject old engines and verify installation, never a full protection verdict. */
export interface PolicyState {
  /** Empty before native installation. */
  userAgent: string;
  /** Absent before installation; validated separately when present. */
  userAgentMetadata?: unknown;
  /** Absent before installation; exact native desktop token once configured. */
  platform?: string;
  /** Exact supported engine contract revision. */
  version: number;
  /** True after any session renderer has started, including worker renderers. */
  rendererStarted: boolean;
  /** Engine-owned system timezone or empty before configuration. */
  timeZone: string;
  /** Engine-owned canonical locale or empty before configuration. */
  locale: string;
  /** Engine-owned logical CPU count or zero before configuration. */
  hardwareConcurrency: number;
  /** Browser-owned language preference list, comma-separated. */
  acceptLanguages: string;
}
