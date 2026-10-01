/**
 * The admin page's numbers, shaped for display: pure functions, tested alone.
 */
import { CENTS_PER_DOLLAR, DAY_CHARS, MIN_BAR_PERCENT, MS_PER_DAY, PERCENT, SECONDS_PER_HOUR } from './constants';

/** A download count as the server keeps it. */
export interface DownloadRow {
  /** The UTC day. */
  day: string;
  /** installer, update or update_check. */
  kind: string;
  /** mac, windows or linux. */
  platform: string;
  /** How many. */
  count: number;
}

/** One day's downloads: installers, updates and update checks, all platforms together. */
export interface DownloadDay {
  /** The UTC day. */
  day: string;
  /** Installers a person downloaded. */
  installer: number;
  /** Updates the app fetched. */
  update: number;
  /** Times the app checked for an update. */
  update_check: number;
}

/** Downloads per day, newest first. */
export function downloadsByDay(rows: DownloadRow[]): DownloadDay[] {
  const days = new Map<string, DownloadDay>();
  for (const r of rows) {
    const d = days.get(r.day) || { day: r.day, installer: 0, update: 0, update_check: 0 };
    if (r.kind === 'installer' || r.kind === 'update' || r.kind === 'update_check') d[r.kind] += Number(r.count) || 0;
    days.set(r.day, d);
  }
  return [...days.values()].sort((a, b) => b.day.localeCompare(a.day));
}

/** Hours with one decimal, from seconds. */
export const hours = (seconds = 0) => (seconds / SECONDS_PER_HOUR).toFixed(1);

/** An ISO time as its day, or a dash. */
export const dayOf = (iso?: string | null) => (iso ? String(iso).slice(0, DAY_CHARS) : '—');

/** The day `days` from now, as a date input takes it. */
export const daysFromNow = (days: number, now = Date.now()) =>
  new Date(now + days * MS_PER_DAY).toISOString().slice(0, DAY_CHARS);

/** A stretch's total against the one before, as the server worked it out. */
export interface Comparison {
  /** This stretch's total. */
  now: number;
  /** The stretch before's total. */
  before: number;
  /** The change in percent, or null when the stretch before had none. */
  change: number | null;
}

/** A change as it reads: "+12%", "-3%", "new" when there was nothing before, or "flat". */
export function changeText(c: Comparison) {
  if (c.change === null) return c.now ? 'new' : 'flat';
  return c.change > 0 ? `+${c.change}%` : c.change < 0 ? `${c.change}%` : 'flat';
}

/** Whether a change went up, down, or neither. */
export function direction(c: Comparison) {
  const sign = c.change ?? c.now;
  return sign > 0 ? 'up' : sign < 0 ? 'down' : 'flat';
}

/** Cents as whole dollars, with thousands separated. */
export const dollars = (cents = 0) => `$${Math.round(cents / CENTS_PER_DOLLAR).toLocaleString('en-US')}`;

/** Each bar's height in percent of the tallest; a day with anything on it gets at least a sliver. */
export function barHeights(values: number[]) {
  const top = Math.max(...values, 0);
  return values.map((v) => (top && v ? Math.max((v / top) * PERCENT, MIN_BAR_PERCENT) : 0));
}
