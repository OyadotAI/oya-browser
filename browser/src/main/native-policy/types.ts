/** Explicit policy subset and browser-owned engine seam; never a full-persona protection verdict. */
/** Immutable settings that the currently implemented native primitives can enforce. */
export interface NativePolicy {
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
