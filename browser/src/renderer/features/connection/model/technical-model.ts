/** Public catalog fields used by the Technical form; provider credentials never appear here. */
export interface TechnicalCatalogEntry {
  /** Stable server provider identifier. */ id: string;
  /** Provider name shown by the server. */ label: string;
  /** Catalog-selected model unless the person explicitly overrides it. */ model: string;
  /** Credential shape used only as placeholder text. */ hint?: string;
  /** Current public choices for the optional model selector. */ models?: {
    /** API model ID. */ id: string;
    /** Human-readable name. */ label: string;
  }[];
}
