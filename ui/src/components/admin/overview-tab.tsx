/**
 * The admin page's first tab: four headline numbers with how they moved,
 * anything that needs attention, each counter's week, and the days folded away.
 */
import { Kpi } from './parts';
import { Daily, Growth, Trend } from './growth-sections';
import { dollars } from './model';
import type { Overview } from './types';

/** Paying accounts by plan, such as "2 developer, 1 team". */
const payingText = (o: Overview) =>
  Object.entries(o.accounts.byPlan || {})
    .map(([plan, n]) => `${n} ${plan}`)
    .join(', ') || 'none';

/** The four numbers that say how the business is doing. */
function Headline({ o }: { /** The overview. */ o: Overview }) {
  const { reach, week } = o.growth;
  const signups = o.accounts.signups.reduce((n, d) => n + d.count, 0);
  const r = o.revenue;
  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <Kpi
        label="Accounts"
        value={o.accounts.total.toLocaleString('en-US')}
        trend={<Trend c={week.signups} against="signups vs last week" />}
        sub={`${signups} signups in 30 days`}
      />
      <Kpi
        label="MRR"
        value={r.enabled ? dollars(r.mrrCents) : 'off'}
        trend={r.enabled && <Trend c={week.revenue_cents} against="paid vs last week" />}
        sub={`Paying: ${payingText(o)}`}
      />
      <Kpi
        label="Active, 7 days"
        value={reach.week}
        trend={<Trend c={week.active} against="vs last week" />}
        sub={`${reach.today} today · ${reach.month} in 30 days`}
      />
      <Kpi label="Browsers now" value={o.fleet.total} sub={`${o.fleet.cloud} in the cloud`} />
    </div>
  );
}

/** What needs a look: failed payments and an unreadable Stripe. Nothing when all is well. */
function Attention({ o }: { /** The overview. */ o: Overview }) {
  const notes = [
    o.accounts.pastDue > 0 && `${o.accounts.pastDue} payment failed`,
    o.installs.overCap > 0 && `${o.installs.overCap} self-hosted install over the free cap, unlicensed`,
    o.revenue.error && `Stripe could not be read: ${o.revenue.error}`,
  ].filter(Boolean) as string[];
  if (!notes.length) return null;
  return (
    <ul className="flex flex-col gap-1 rounded-md border border-red/30 bg-red/5 px-3 py-2 text-xs text-red">
      {notes.map((n) => (
        <li key={n}>{n}</li>
      ))}
    </ul>
  );
}

/** The overview tab. */
export function OverviewTab({ o }: { /** The overview. */ o: Overview }) {
  return (
    <div className="flex flex-col gap-6">
      <Headline o={o} />
      <Attention o={o} />
      <Growth o={o} />
      <Daily o={o} />
    </div>
  );
}
