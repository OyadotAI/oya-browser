/** Pure Keel usage calculations preserve unavailable measurements and installation-scoped project identity. */
import { DAYS_SHOWN, DAY_CHARS, MS_PER_DAY } from './constants.ts';

/** A UTC date string, used consistently for storage and the administrator's date range. */
const dayOf = (time: number) => new Date(time).toISOString().slice(0, DAY_CHARS);

/** Thirty completed UTC days; today's unfinished counters never enter the usage total. */
export function keelPeriod(now: number) {
  const end = Date.parse(`${dayOf(now)}T00:00:00.000Z`);
  return { from: dayOf(end - DAYS_SHOWN * MS_PER_DAY), to: dayOf(end - MS_PER_DAY), days: DAYS_SHOWN };
}

/** Only actual measured integers count toward runtime coverage. */
const runtimeMeasured = (row) => Number.isSafeInteger(row.runtime_ms) && row.runtime_ms >= 0;

/** Distinct IDs are namespaced by installation, including when two machines use the same repository. */
function projects(rows): number | null {
  const measured = rows.filter((row) => Array.isArray(row.project_ids));
  if (!measured.length) return null;
  return new Set(measured.flatMap((row) => row.project_ids.map((id: string) => `${row.install_id}:${id}`))).size;
}

/** Absence and measured zero remain different in both summaries and chart days. */
export function keelTotals(rows: Record<string, any>[]) {
  const runtime = rows.filter(runtimeMeasured);
  return {
    ...measurementCoverage(rows, runtime),
    runtime_ms: runtimeTotal(runtime),
    projects: projects(rows),
    steps: rows.reduce((sum, row) => sum + (Number(row.steps) || 0), 0),
  };
}

/** Coverage remains independent for runtime and project snapshots. */
const measurementCoverage = (rows, runtime) => ({
  reports: rows.length,
  runtime_reports: runtime.length,
  project_reports: rows.filter((row) => Array.isArray(row.project_ids)).length,
});

/** Missing runtime remains null even when other measurements exist on the same reports. */
const runtimeTotal = (rows) => (rows.length ? rows.reduce((sum, row) => sum + row.runtime_ms, 0) : null);

/** A complete date axis with missing days left unavailable rather than interpolated. */
export function keelDays(rows: Record<string, any>[], from: string) {
  const start = Date.parse(`${from}T00:00:00.000Z`);
  return Array.from({ length: DAYS_SHOWN }, (_, index) => {
    const day = dayOf(start + index * MS_PER_DAY);
    return { day, ...keelTotals(rows.filter((row) => row.day === day)) };
  });
}
