/** Currency-specific financial values distinguish complete zero from missing or incomplete retrieval. */

/** Stripe charge/invoice units are explicit; no consumer may assume every amount is cents. */
export interface Money {
  /** Lowercase source currency. */ currency: string;
  /** Rounded only after aggregating monthly equivalents. */ amountMinor: number;
  /** Verified API conversion exponent. */ minorUnitExponent: number;
}

/** An incomplete traversal never exposes a subtotal as a complete measurement. */
export interface FinancialMetric {
  /** Completeness is independent for recurring prices and paid invoices. */ state:
    'complete' | 'incomplete' | 'unavailable' | 'not_configured';
  /** Empty is measured zero; null is no authoritative value. */ values: Money[] | null;
  /** Coverage or recovery explanation. */ reason: string;
}

/** Paid invoices use final paid_at, not creation time or individual cash-receipt events. */
export interface PaidInvoices extends FinancialMetric {
  /** Matching-currency prior period, or unavailable. */ previousValues: Money[] | null;
  /** Completed UTC dates with currency-specific values. */ days:
    { /** Usage date. */ day: string; /** Measured currency amounts. */ values: Money[] }[] | null;
}
