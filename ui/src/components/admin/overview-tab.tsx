/** Account activity, product usage and distribution use matching completed UTC periods. */
import { Kpi, Section, Table } from './parts';
import { Daily, Growth, Trend } from './growth-sections';
import { FinancialCards, FinancialStatus, InvoiceDetail } from './financial-cards';
import type { Overview } from './types';
import type { AdminState } from './use-admin';

/** Stock and activity cards describe their own populations rather than unrelated trends. */
function Headline({ o, s }: { /** Core activity snapshot. */ o: Overview; /** Independent finance. */ s: AdminState }) {
  const { reach, week } = o.growth;
  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
      <Kpi
        label="Total accounts · now"
        value={o.accounts.total.toLocaleString('en-US')}
        sub="All registered accounts"
      />
      <Kpi
        label="Active accounts · 7 days"
        value={reach.week}
        trend={<Trend c={week.active} against="vs preceding 7 days" />}
        sub={`${reach.month} distinct accounts in 30 completed days`}
      />
      <FinancialCards s={s} />
    </div>
  );
}

/** Observed subscription states and installation reports are support signals, not payment proof. */
function Attention({ o }: { /** Core activity snapshot. */ o: Overview }) {
  const notes = [
    o.accounts.pastDue > 0 && `${o.accounts.pastDue} subscriptions past due`,
    o.installs.overCap > 0 &&
      `${o.installs.overCap} recently reporting self-hosted installs above the free cap with no reported license ID`,
  ].filter(Boolean);
  return notes.length ? (
    <ul className="space-y-1 rounded-md border border-red/30 bg-red/5 px-3 py-2 text-xs text-red">
      {notes.map((note) => (
        <li key={String(note)}>{note}</li>
      ))}
    </ul>
  ) : null;
}

/** Subscription records are separated by plan and state, never presented as paid customers. */
function Subscriptions({ o }: { /** Recorded subscription states. */ o: Overview }) {
  return (
    <Section title="Subscription status · now" hint="Recorded billing states">
      {o.accounts.statuses.length ? (
        <Table
          head={['Plan', 'Active', 'Trialing', 'Past due']}
          rows={o.accounts.statuses.map((row) => [row.plan, row.active, row.trialing, row.past_due])}
        />
      ) : (
        <p className="text-xs text-text-dim">No subscription records.</p>
      )}
    </Section>
  );
}

/** Finance remains useful even if the independent core activity request is unavailable. */
export function OverviewTab({
  o,
  s,
}: {
  /** Core data may be unavailable. */ o: Overview | null;
  /** Independent feeds. */ s: AdminState;
}) {
  return (
    <div className="flex flex-col gap-6">
      {o ? (
        <Headline o={o} s={s} />
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          <FinancialCards s={s} />
        </div>
      )}
      <FinancialStatus s={s} />
      <InvoiceDetail s={s} />
      {o && (
        <>
          <Attention o={o} />
          <Growth o={o} />
          <Subscriptions o={o} />
          <Daily o={o} />
        </>
      )}
    </div>
  );
}
