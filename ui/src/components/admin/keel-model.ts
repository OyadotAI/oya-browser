/** Formats measured Keel usage without disguising missing reports as zero or verified keys. */
import { MIN_HOUR_DISPLAY, MS_PER_HOUR, RUNTIME_DECIMALS } from './constants';
import type { KeelUsage } from './keel-types';

/** Counts use a stable locale for compact, readable administrative tables. */
export const keelCount = (count: number) => count.toLocaleString('en-US');

/** Very small positive runtimes stay visibly different from measured zero. */
export function runtimeText(milliseconds: number | null): string {
  if (milliseconds === null) return '—';
  const hours = milliseconds / MS_PER_HOUR;
  return hours > 0 && hours < MIN_HOUR_DISPLAY ? '<0.1 h' : `${hours.toFixed(RUNTIME_DECIMALS)} h`;
}

/** Null measurements get an explicit unavailable description for assistive technology. */
export const measuredLabel = (value: number | null, text: string) => (value === null ? 'Not reported' : text);

/** Coverage counts actual daily snapshots, excluding presence-only markers. */
export function coverageText(measured: number, reports: number): string {
  if (!measured) return 'Not reported';
  const coverage = `${keelCount(measured)} of ${keelCount(reports)} daily reports`;
  return measured < reports ? `Partial coverage · ${coverage}` : `${coverage} measured`;
}

/** Unknown and mixed instrumentation each explain precisely what the administrator is seeing. */
export function measurementNote(usage: KeelUsage): string {
  if (!usage.runtime_reports && !usage.project_reports)
    return 'Hours and project counts will appear as updated Keel installations report. Existing reports do not include these measurements.';
  if (usage.runtime_reports < usage.reports || usage.project_reports < usage.reports)
    return 'Runtime and project totals include measured reports only. Some reports do not include these measurements.';
  return 'Agent runtime includes tools and approval waits during supervised runs. Concurrent lanes add agent-hours; idle sessions, verification and automatic commits are excluded.';
}
