/**
 * The fleet strip: totals, health, throughput, providers and personas above
 * the table. Its chips and counts live in `fleet/`.
 */
'use client';

import type { Fleet, Health } from './types';
import Throughput, { type ThroughputProps } from './fleet/throughput';
import { HealthChips, PersonaChips, ProviderChips } from './fleet/strip-chips';
import { useStripCounts } from './fleet/use-strip-counts';

/** What the fleet table is narrowed to; shared by the strip, the table and the page. */
export interface FleetFilter {
  /** Only browsers in this health. */
  health: Health | null;
  /** Only browsers on this provider. */
  provider: string | null;
  /** Only browsers under this persona (its name, or id when unnamed). */
  persona: string | null;
  /** Free text matched against name, id, url, persona and provider. */
  text: string;
}

/** The latest fleet poll and the filter. */
interface Props {
  /** Latest fleet summary, or null before the first poll. */
  fleet: Fleet | null;
  /** Command counters from the previous poll, for a per-minute rate. */
  rate: ThroughputProps['rate'];
  /** Current filter. */
  filter: FleetFilter;
  /** Merges a change into the filter. */
  onFilter: (next: Partial<FleetFilter>) => void;
}

/**
 * The fleet at a glance, and every number is a filter. "errors 12" is not a
 * statistic to admire — click it and the table shows those twelve.
 */
export default function FleetStrip({ fleet, rate, filter, onFilter }: Props) {
  const b = fleet?.browsers;
  const { providers, topPersonas, restPersonas } = useStripCounts(b);
  return (
    <div className="border-b border-border bg-bg-card/40">
      <div className="flex flex-wrap items-center gap-x-6 gap-y-2 px-4 py-2.5 lg:px-6">
        <div className="flex items-baseline gap-2">
          <span className="font-display text-2xl font-bold num text-text">{b?.total ?? '—'}</span>
          <span className="text-[12px] uppercase tracking-[0.12em] text-text-muted">browsers</span>
        </div>
        <HealthChips b={b} filter={filter} onFilter={onFilter} />
        <Throughput rate={rate} b={b} />
        <div className="ml-auto" />
        {providers.length > 0 && <ProviderChips entries={providers} filter={filter} onFilter={onFilter} />}
        {topPersonas.length > 0 && (
          <PersonaChips entries={topPersonas} rest={restPersonas} filter={filter} onFilter={onFilter} />
        )}
      </div>
    </div>
  );
}
