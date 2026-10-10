/** Immutable user-agent metadata owned by the native policy snapshot, not the caller. */
/** One ordered native client-hint brand/version pair. */
export interface NativeBrand {
  /** Display name, bounded to the engine's printable ASCII contract. */
  readonly brand: string;
  /** Major or full version depending on the containing list. */
  readonly version: string;
}
/** Exact fields supported by the native metadata primitive. */
export interface NativeMetadata {
  /** Ordered low-entropy brands. */
  readonly brands: readonly NativeBrand[];
  /** Matching ordered names with full versions. */
  readonly fullVersionList: readonly NativeBrand[];
  /** Full engine version supplied by the identity builder. */
  readonly fullVersion: string;
  /** User-agent metadata OS name, distinct from navigator.platform. */
  readonly platform: string;
  /** User-agent metadata OS version. */
  readonly platformVersion: string;
  /** Web-exposed architecture, not an OS scheduling control. */
  readonly architecture: string;
  /** Device model, normally empty on desktop. */
  readonly model: string;
  /** Native low-entropy mobile flag. */
  readonly mobile: boolean;
  /** Web-exposed architecture bitness. */
  readonly bitness: string;
  /** Whether the identity represents Windows-on-Windows. */
  readonly wow64: boolean;
  /** Ordered native form-factor tokens. */
  readonly formFactors: readonly string[];
}
