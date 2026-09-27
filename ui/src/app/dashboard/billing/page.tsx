/**
 * The billing page: the plan and its period, this period's use against what
 * it includes, the next invoice line by line, and every invoice issued. Its
 * state lives in `components/dashboard/billing/use-billing-page.ts`.
 */
'use client';

import Link from 'next/link';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { useBillingPage } from '@/components/dashboard/billing/use-billing-page';
import { History, NextInvoice, PlanCard, Usage } from '@/components/dashboard/billing/billing-sections';

/** A short message in place of the page. */
const Notice = ({ children }: { /** The message. */ children: React.ReactNode }) => (
  <p className="p-6 text-sm text-text-muted">{children}</p>
);

/** What the page shows once loaded, or why it cannot. */
function Body({ s }: { /** The page's state. */ s: ReturnType<typeof useBillingPage> }) {
  if (s.loading || (s.signedIn && !s.data))
    return (
      <Notice>
        <Loader2 className="h-5 w-5 animate-spin" />
      </Notice>
    );
  if (!s.signedIn)
    return (
      <Notice>
        Billing belongs to an account.{' '}
        <Link className="text-accent" href="/login">
          Sign in
        </Link>{' '}
        to see it.
      </Notice>
    );
  if (s.data?.error) return <Notice>{s.data.error}</Notice>;
  const b = s.data?.billing;
  if (!b?.enabled) return <Notice>This server has no plans: it is self-hosted.</Notice>;
  return (
    <>
      <PlanCard b={b} s={s} />
      <Usage b={b} />
      {s.data?.upcoming && <NextInvoice u={s.data.upcoming} />}
      <History invoices={s.data?.invoices || []} />
    </>
  );
}

/** The billing page. */
export default function BillingPage() {
  const s = useBillingPage();
  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col gap-4 bg-bg p-4 lg:p-6">
      <Link href="/dashboard" className="flex items-center gap-1.5 text-xs text-text-dim hover:text-text">
        <ArrowLeft className="h-3.5 w-3.5" /> Dashboard
      </Link>
      <h1 className="text-lg font-semibold text-text">Plan &amp; billing</h1>
      <Body s={s} />
    </main>
  );
}
