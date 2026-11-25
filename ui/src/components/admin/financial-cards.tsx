/** Independent finance cards disclose scope, coverage and source freshness beside their amounts. */
import { Fold, Kpi, Table } from './parts';
import { FeedNotice } from './feed-state';
import { financialText, moneyText } from './financial-model';
import { utcTime } from './model';
import type { AdminState } from './use-admin';

/** Financial retrieval has its own refresh; a Stripe error never hides account activity. */
export function FinancialCards({ s }: { /** Independent administrative feeds. */ s: AdminState }) {
  const feed = s.financialFeed;
  const financial = feed.value;
  return (
    <>
      <Kpi
        label="Active fixed-price MRR · now"
        value={financialText(financial?.mrr)}
        sub={financial?.mrr.reason || 'Active per-unit month/year prices; metered charges excluded.'}
      />
      <Kpi
        label="Paid invoice totals · 7 days"
        value={financialText(financial?.paidInvoices)}
        sub={
          financial?.paidInvoices.reason ||
          (financial
            ? `${financial.periods.week.from}–${financial.periods.week.to} UTC · final payment date`
            : 'Awaiting Stripe snapshot')
        }
      />
    </>
  );
}

/** Freshness and recovery belong to finance rather than the core activity request. */
export function FinancialStatus({ s }: { /** Independent administrative feeds. */ s: AdminState }) {
  const feed = s.financialFeed;
  return (
    <div className="space-y-2 text-xs text-text-dim">
      <div className="flex flex-wrap items-center gap-3">
        <span>
          {feed.value ? `Finance updated ${utcTime(feed.value.generated_at)}` : 'Finance has no snapshot yet.'}
        </span>
        <button
          type="button"
          disabled={feed.loading}
          onClick={feed.refresh}
          className="min-h-11 text-accent hover:underline disabled:opacity-50"
        >
          Refresh finance
        </button>
      </div>
      <FeedNotice feed={feed} available={Boolean(feed.value)} name="Finance" />
      <p>
        Paid invoice totals use amount paid at the final paid date. They do not measure individual cash receipts.
        Currencies remain separate.
      </p>
    </div>
  );
}

/** Exact invoice detail preserves currencies and exposes the comparison period without borrowing MRR's label. */
export function InvoiceDetail({ s }: { /** Independent finance feed. */ s: AdminState }) {
  const snapshot = s.financialFeed.value;
  const invoice = snapshot?.paidInvoices;
  if (!snapshot || invoice?.state !== 'complete') return null;
  const prior = invoice.previousValues?.map(moneyText).join(' · ') || '0';
  return (
    <Fold label="Paid invoices by final payment date · 30 completed UTC days">
      <p className="px-2 pb-2 text-xs text-text-muted">
        Preceding 7 days · {snapshot.periods.previousWeek.from}–{snapshot.periods.previousWeek.to} UTC: {prior}
      </p>
      <Table
        head={['Day · UTC', 'Paid invoice totals']}
        rows={[...(invoice.days || [])].reverse().map((day) => [day.day, day.values.map(moneyText).join(' · ') || '0'])}
      />
    </Fold>
  );
}
