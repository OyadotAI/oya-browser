/**
 * The billing page's state: the plan and its use, the next invoice, the
 * invoices already issued, and the two ways out to Stripe (subscribe, manage).
 */
import { useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { billingCheckout, billingInvoices, billingPortal, billingUpcoming, getBilling } from '@/lib/api';
import { useLeave, type Billing } from './use-plan';

/** An issued invoice. */
export interface Invoice {
  /** Its id. */
  id: string;
  /** Its number, such as OYA-0001. */
  number: string | null;
  /** draft, open, paid, void or uncollectible. */
  status: string;
  /** When it was issued. */
  created: string;
  /** Its currency. */
  currency: string;
  /** Its total, in cents. */
  total: number;
  /** Its page on Stripe. */
  url: string | null;
  /** Its PDF. */
  pdf: string | null;
}

/** The next invoice as it stands. */
export interface Upcoming {
  /** Its total so far, in cents. */
  total: number;
  /** Its currency. */
  currency: string;
  /** When it is due. */
  date: string | null;
  /** What it is for, line by line. */
  lines: {
    /** What the line is. */ description: string;
    /** How many. */ quantity: number | null;
    /** In cents. */ amount: number;
  }[];
}

/** Everything the page loaded, or why it could not. */
interface Loaded {
  /** The plan and its use. */
  billing: Billing | null;
  /** Issued invoices. */
  invoices: Invoice[];
  /** The next invoice. */
  upcoming: Upcoming | null;
  /** Why loading failed; empty when it did not. */
  error: string;
}

/** Loads it all for `token`; a failure is said, not thrown. */
async function load(token: string): Promise<Loaded> {
  try {
    const all = await Promise.all([getBilling(token), billingInvoices(token), billingUpcoming(token)]);
    return { billing: all[0], invoices: all[1].invoices, upcoming: all[2].upcoming, error: '' };
  } catch (e) {
    return { ...EMPTY, error: e instanceof Error ? e.message : 'Could not load billing' };
  }
}

/** Nothing loaded. */
const EMPTY: Loaded = { billing: null, invoices: [], upcoming: null, error: '' };

/** The loaded data, for the signed-in person. */
function useLoaded(token: string | null) {
  const [data, setData] = useState<Loaded | null>(null);
  useEffect(() => {
    if (token) void load(token).then(setData);
  }, [token]);
  return data;
}

/** Everything the billing page shows and does. */
export function useBillingPage() {
  const { token, user, loading } = useAuth();
  const data = useLoaded(token);
  const { error, leaveFor } = useLeave();
  const upgrade = (plan: string) => (token ? leaveFor(() => billingCheckout(token, plan)) : undefined);
  const manage = () => (token ? leaveFor(() => billingPortal(token)) : undefined);
  return { signedIn: Boolean(user), loading, data, error, upgrade, manage };
}

/** The billing page's state, as its sections take it. */
export type BillingPageState = ReturnType<typeof useBillingPage>;
