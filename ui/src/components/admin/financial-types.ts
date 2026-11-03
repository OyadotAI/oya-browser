/** Financial state and API charge units remain explicit in every staff money display. */
import type { Periods } from './types';

/** Independent currency amount, never combined with another currency. */
export interface Money {
  /** ISO source code. */ currency: string;
  /** Source API units after aggregation rounding. */ amountMinor: number;
  /** Verified API conversion exponent. */ minorUnitExponent: number;
}

/** Failed or incomplete retrieval cannot look like successful zero. */
export interface FinancialMetric {
  /** Complete and measured, or why no total is available. */ state:
    'complete' | 'incomplete' | 'unavailable' | 'not_configured';
  /** Empty means successful zero; null means no authoritative amount. */ values: Money[] | null;
  /** Recovery/coverage explanation. */ reason: string;
}

/** Paid invoice totals use final payment date, not invoice creation or individual cash receipts. */
export interface PaidInvoices extends FinancialMetric {
  /** Matching prior seven-day period. */ previousValues: Money[] | null;
  /** Completed daily currency amounts. */ days:
    { /** UTC date. */ day: string; /** Separate currency amounts. */ values: Money[] }[] | null;
}

/** This feed has no dependency on core activity or license-list availability. */
export interface FinancialSnapshot {
  /** Snapshot request time, explicitly UTC. */ generated_at: string;
  /** Exact date scopes shared with core activity. */ periods: Periods;
  /** Active fixed per-unit month/year monthly equivalent. */ mrr: FinancialMetric;
  /** Completed invoices grouped by final paid_at. */ paidInvoices: PaidInvoices;
}
