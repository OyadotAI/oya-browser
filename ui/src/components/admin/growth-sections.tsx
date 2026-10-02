/**
 * The admin overview's growth sections: each counter's last 7 days with its
 * week-over-week and day-over-day change and a bar per day, and the days
 * themselves as a table, folded away until asked for.
 */
import { Fold, Section, Table } from './parts';
import { barHeights, changeText, direction, dollars, hours, type Comparison } from './model';
import type { Counted, Day, Overview } from './types';

/** A counter the page shows: its key, its name and how its numbers read. */
interface Metric {
  /** The counter. */
  key: Counted;
  /** What it counts. */
  label: string;
  /** A number of it as it reads. */
  format: (n: number) => string;
}

/** A plain count, with thousands separated. */
const count = (n: number) => n.toLocaleString('en-US');

/** Every counter, in the order the page shows them. */
const METRICS: Metric[] = [
  { key: 'signups', label: 'Signups', format: count },
  { key: 'active', label: 'Person-days active', format: count },
  { key: 'agent_steps', label: 'Agent steps', format: count },
  { key: 'cloud_seconds', label: 'Cloud hours', format: (n) => hours(n) },
  { key: 'browsers_started', label: 'Browsers started', format: count },
  { key: 'commands', label: 'Commands', format: count },
  { key: 'installers', label: 'Installer downloads', format: count },
  { key: 'update_checks', label: 'Desktop update checks', format: count },
  { key: 'new_installs', label: 'New self-hosted installs', format: count },
  { key: 'revenue_cents', label: 'Revenue', format: dollars },
];

/** Each direction's ink and arrow: the arrow and the words carry it, not the color alone. */
const LOOK = {
  up: { ink: 'text-accent', arrow: '▲' },
  down: { ink: 'text-red', arrow: '▼' },
  flat: { ink: 'text-text-dim', arrow: '' },
};

/** A change, such as "▲ +12% vs last week". */
export function Trend({
  c,
  against,
}: {
  /** The comparison. */ c: Comparison;
  /** What it is against. */ against: string;
}) {
  const look = LOOK[direction(c)];
  return (
    <span className={`text-[11px] ${look.ink}`}>
      {look.arrow} {changeText(c)} <span className="text-text-dim">{against}</span>
    </span>
  );
}

/** One bar per day, oldest left; each says its day and number on hover. */
function Bars({ days, m }: { /** The days. */ days: Day[]; /** The counter. */ m: Metric }) {
  const heights = barHeights(days.map((d) => d[m.key]));
  return (
    <div
      className="flex h-10 items-end gap-[2px]"
      role="img"
      aria-label={`${m.label} per day, last ${days.length} days`}
    >
      {days.map((d, i) => (
        <div key={d.day} className="flex h-full flex-1 items-end" title={`${d.day}: ${m.format(d[m.key])}`}>
          <div className="w-full rounded-t-[2px] bg-accent/80 hover:bg-accent" style={{ height: `${heights[i]}%` }} />
        </div>
      ))}
    </div>
  );
}

/** One counter: its last 7 days, the change on the week and on the day, and its days as bars. */
function MetricCard({ o, m }: { /** The overview. */ o: Overview; /** The counter. */ m: Metric }) {
  const week = o.growth.week[m.key];
  const day = o.growth.day[m.key];
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border bg-bg-card p-3">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[11px] text-text-dim">{m.label}</span>
        <span className="text-lg font-semibold text-text tabular-nums">{m.format(week.now)}</span>
      </div>
      <Bars days={o.growth.days} m={m} />
      <div className="flex flex-wrap justify-between gap-x-2 text-[11px]">
        <Trend c={week} against="vs last week" />
        <span className="text-text-dim">
          yesterday {m.format(day.now)} <Trend c={day} against="" />
        </span>
      </div>
    </div>
  );
}

/** Every counter's card. */
export function Growth({ o }: { /** The overview. */ o: Overview }) {
  return (
    <Section title="Last 7 days" hint={`bars: ${o.growth.days.length} days, today on the right`}>
      <div className="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-5">
        {METRICS.map((m) => (
          <MetricCard key={m.key} o={o} m={m} />
        ))}
      </div>
    </Section>
  );
}

/** Every day shown, newest first, with every counter, folded away. */
export function Daily({ o }: { /** The overview. */ o: Overview }) {
  const rows = [...o.growth.days].reverse().map((d) => [d.day, ...METRICS.map((m) => m.format(d[m.key]))]);
  return (
    <Fold label="Day by day (UTC)">
      <Table head={['Day', ...METRICS.map((m) => m.label)]} rows={rows} />
    </Fold>
  );
}
