/**
 * Formatting for tables that refresh every few seconds: relative times and
 * shortened ids.
 */
import {
  JUST_NOW_SECONDS,
  MS_PER_SECOND,
  SECONDS_PER_DAY,
  SECONDS_PER_HOUR,
  SECONDS_PER_MINUTE,
  SHORT_ID_KEEP,
  SHORT_ID_MAX,
} from './constants';

/** Relative time that reads like a person wrote it, for tables that refresh every few seconds. */
export function ago(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return '—';
  const sec = Math.max(0, Math.floor((now - new Date(iso).getTime()) / MS_PER_SECOND));
  if (sec < JUST_NOW_SECONDS) return 'now';
  if (sec < SECONDS_PER_MINUTE) return `${sec}s`;
  if (sec < SECONDS_PER_HOUR) return `${Math.floor(sec / SECONDS_PER_MINUTE)}m`;
  if (sec < SECONDS_PER_DAY) return hoursAndMinutes(sec);
  return `${Math.floor(sec / SECONDS_PER_DAY)}d`;
}

/** "3h 12m" for a span under a day. */
function hoursAndMinutes(sec: number): string {
  return `${Math.floor(sec / SECONDS_PER_HOUR)}h ${Math.floor((sec % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE)}m`;
}

/** A long id cut to its first characters, so a table column stays narrow. */
export function shortId(id: string): string {
  return id.length > SHORT_ID_MAX ? `${id.slice(0, SHORT_ID_KEEP)}…` : id;
}
