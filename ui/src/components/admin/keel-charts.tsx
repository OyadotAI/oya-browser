/** Daily measured bars distinguish absent, zero and partial reports, with exact values in a companion table. */
import { Fold, Section, Table } from './parts';
import { barHeights } from './model';
import { coverageText, keelCount, runtimeText } from './keel-model';
import type { KeelDay, KeelSnapshot } from './keel-types';
import { KEEL_DAILY_NUMERIC } from './constants';

/** A chart's measured field, expressed in the same units as its daily table. */
type Field = 'runtime_ms' | 'steps';

/** Missing steps also require a report, so a gap cannot masquerade as observed zero. */
const valueOf = (day: KeelDay, field: Field) => (day.reports ? day[field] : null);

/** Exact text is available without relying on bar color or a pointer hover. */
const valueText = (value: number | null, field: Field) =>
  field === 'runtime_ms' ? runtimeText(value) : value === null ? 'Not reported' : keelCount(value);

/** Compact bars follow the admin's existing styling; dashed baselines identify missing data. */
function Bars({ days, field }: { /** Chronological days. */ days: KeelDay[]; /** Metric. */ field: Field }) {
  const heights = barHeights(days.map((day) => valueOf(day, field) ?? 0));
  return (
    <div
      className="flex h-16 items-end gap-0.5"
      role="img"
      aria-label="Daily usage; dashed markers mean not reported. Exact values follow in Day by day (UTC)."
    >
      {days.map((day, index) => (
        <div
          key={day.day}
          className="flex h-full min-w-0 flex-1 items-end"
          title={`${day.day}: ${valueText(valueOf(day, field), field)}; ${coverageText(field === 'runtime_ms' ? day.runtime_reports : day.reports, day.reports)}`}
        >
          {valueOf(day, field) === null ? (
            <div className="w-full border-b border-dashed border-text-dim" />
          ) : (
            <div
              className={`min-h-px w-full rounded-t-sm bg-accent/80 ${field === 'runtime_ms' && day.runtime_reports < day.reports ? 'border-t-2 border-dashed border-text' : ''}`}
              style={{ height: `${heights[index]}%` }}
            />
          )}
        </div>
      ))}
    </div>
  );
}

/** One metric stays compact and carries a visible explanation of incomplete coverage. */
function Chart({
  data,
  field,
  label,
}: {
  /** Snapshot. */ data: KeelSnapshot;
  /** Metric. */ field: Field;
  /** Heading. */ label: string;
}) {
  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border bg-bg-card p-3">
      <h3 className="text-xs font-medium text-text">{label}</h3>
      <Bars days={data.days} field={field} />
      <p className="text-[11px] text-text-dim">Dashed baseline: not reported. Dashed bar top: partial coverage.</p>
    </div>
  );
}

/** Exact UTC values and independent measurement coverage remain accessible beneath the charts. */
function DailyTable({ data }: { /** Snapshot. */ data: KeelSnapshot }) {
  const rows = [...data.days]
    .reverse()
    .map((day) => [
      day.day,
      runtimeText(day.runtime_ms),
      day.reports ? keelCount(day.steps) : '—',
      day.projects === null ? '—' : keelCount(day.projects),
      coverageText(day.runtime_reports, day.reports),
      coverageText(day.project_reports, day.reports),
    ]);
  return (
    <Fold label="Day by day (UTC)">
      <Table
        head={['Day', 'Agent runtime', 'Reported steps', 'Active projects', 'Runtime coverage', 'Project coverage']}
        rows={rows}
        numeric={KEEL_DAILY_NUMERIC}
      />
    </Fold>
  );
}

/** Two compact charts and their shared exact table retain the established admin rhythm. */
export function KeelCharts({ data }: { /** Snapshot. */ data: KeelSnapshot }) {
  return (
    <Section title="Daily usage" hint={`${data.period.from} – ${data.period.to} · completed UTC days`}>
      <div className="grid gap-3 md:grid-cols-2">
        <Chart data={data} field="runtime_ms" label="Agent runtime" />
        <Chart data={data} field="steps" label="Reported steps" />
      </div>
      <p className="text-xs text-text-dim">Older releases may undercount steps.</p>
      <DailyTable data={data} />
    </Section>
  );
}
