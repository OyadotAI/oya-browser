/**
 * A past time as people say it ("just now", "5 minutes ago", "3 days ago"),
 * for TimeAgo. Pure, so it is tested on its own.
 */
import { RendererConstants as C } from '../core/constants.ts';

/** `at` (epoch milliseconds) as people say it: "just now", "5 minutes ago", "3 days ago". */
export function ago(at: number, now = Date.now()): string {
  const seconds = Math.round((now - at) / C.MS_PER_SECOND);
  if (seconds < C.JUST_NOW_SECONDS) return 'just now';
  const units = Object.entries(C.TIME_UNIT_SECONDS) as [Intl.RelativeTimeFormatUnit, number][];
  const [unit, size] = units.find(([, length]) => seconds >= length) ?? units[units.length - 1];
  return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(-Math.round(seconds / size), unit);
}
