/**
 * Pure formatting and classification for the Control tab: numbers, sizes,
 * durations, tones and the request bodies built from forms.
 */
import {
  BAR_BAD_PCT,
  BAR_WARN_PCT,
  BYTE_UNITS,
  BYTES_PER_KB,
  ERROR_RATE_BAD,
  ERROR_RATE_WARN,
  FRAME_GAP_DEFAULT_MS,
  FRAME_GAP_MAX_MS,
  FRAME_GAP_MIN_MS,
  LOW_REMAINING_SHARE,
  PERCENT,
  SECONDS_PER_HOUR,
  SECONDS_PER_MINUTE,
  USAGE_ROWS,
} from './constants';
import type { AuditEvent, Frame, Limit, Provider, ProviderDraft, Usage } from './types';

/** How a stat is coloured. */
export type Tone = 'normal' | 'warn' | 'bad' | 'good';

/** `commandErrors` → `Command errors`, `chat_tokens` → `Chat tokens`. */
export const metricLabel = (name: string) => {
  const text = name.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/_/g, ' ');
  return text[0].toUpperCase() + text.slice(1).toLowerCase();
};

/** A localised number, or a dash when there is none. */
export const num = (n: number | null | undefined, digits = 0) =>
  n == null ? '—' : n.toLocaleString(undefined, { maximumFractionDigits: digits });

/** A byte count in the largest unit that keeps it above one. */
export const bytes = (n: number) => {
  if (!n) return '0 B';
  const i = Math.min(BYTE_UNITS.length - 1, Math.floor(Math.log(n) / Math.log(BYTES_PER_KB)));
  return `${(n / BYTES_PER_KB ** i).toFixed(i ? 1 : 0)} ${BYTE_UNITS[i]}`;
};

/** Seconds as `42s`, `3m 5s` or `2h 10m`. */
export const duration = (s: number) => {
  if (s < SECONDS_PER_MINUTE) return `${Math.round(s)}s`;
  if (s < SECONDS_PER_HOUR) return `${Math.floor(s / SECONDS_PER_MINUTE)}m ${Math.round(s % SECONDS_PER_MINUTE)}s`;
  return `${Math.floor(s / SECONDS_PER_HOUR)}h ${Math.floor((s % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE)}m`;
};

/** Text colour for a stat's tone. */
export const TONE_CLASS: Record<Tone, string> = {
  bad: 'text-red',
  warn: 'text-yellow',
  good: 'text-accent',
  normal: 'text-text',
};

/** The hour's command, error and throttling figures from the usage counters. */
export const healthFigures = (u: Usage | undefined) => {
  const commands = u?.commands ?? 0;
  const errors = u?.command_errors ?? 0;
  const errorRate = commands ? (errors / commands) * PERCENT : 0;
  const throttled = (u?.rate_limited ?? 0) + (u?.quota_denied ?? 0);
  return { commands, errors, errorRate, throttled };
};

/** Red past the bad error rate, yellow past the warning one, green otherwise. */
export const errorRateTone = (rate: number): Tone =>
  rate > ERROR_RATE_BAD ? 'bad' : rate > ERROR_RATE_WARN ? 'warn' : 'good';

/** Capacity use as a percentage, capped at 100. */
export const usedPct = (used: number, capacity: number) =>
  capacity ? Math.min(PERCENT, (used / capacity) * PERCENT) : 0;

/** Bar colour for a capacity percentage. */
export const barTone = (pct: number) =>
  pct > BAR_BAD_PCT ? 'bg-red-500' : pct > BAR_WARN_PCT ? 'bg-amber-500' : 'bg-accent';

/** A live limit whose remaining allowance is nearly spent. */
export const limitIsLow = (l: Limit) => !l.disabled && l.remaining < (l.burst ?? 0) * LOW_REMAINING_SHARE;

/** One-line state of a provider card. */
export const providerStatus = (p: Provider) =>
  p.totalSessions === 0 && p.totalFailures === 0
    ? 'Not yet connected'
    : p.healthy
      ? 'Ready for connections'
      : 'Connection failed · retrying after cooldown';

/** POST /gateway/providers body from the form; key order matches what the server has always received. */
export const providerBody = (draft: ProviderDraft) => ({
  name: draft.name.trim(),
  ...(draft.type !== 'cdp' && draft.apiKey.trim() ? { apiKey: draft.apiKey.trim() } : {}),
  type: draft.type,
  ...(draft.type === 'cdp' ? { wsUrl: draft.wsUrl.trim() } : {}),
  maxConcurrent: Number(draft.maxConcurrent),
  priority: Number(draft.priority),
  weight: Number(draft.weight),
});

/** Audit target as `type id`, with a long id cut short. */
export const auditTarget = (e: AuditEvent, idChars: number) =>
  e.target_type ? `${e.target_type}${e.target_id ? ` ${String(e.target_id).slice(0, idChars)}` : ''}` : '—';

/** Colour for an audit outcome. */
export const outcomeClass = (outcome: string) =>
  outcome === 'ok' ? 'text-accent' : outcome === 'denied' ? 'text-yellow' : 'text-red';

/** Pause before the next recording frame: their real gap, clamped to a watchable range. */
export const frameGap = (frames: Frame[], index: number) =>
  Math.max(
    FRAME_GAP_MIN_MS,
    Math.min(FRAME_GAP_MAX_MS, (frames[index + 1]?.t ?? 0) - (frames[index]?.t ?? 0) || FRAME_GAP_DEFAULT_MS),
  );

/** Usage rows in table order, with their raw counts; the two formatted rows carry none. */
export const usageRows = (u: Usage): [string, number | null][] =>
  USAGE_ROWS.map(([label, key]) => [label, key ? u[key] : null]);

/** A usage cell's text: time and bytes are formatted, the rest are counts. */
export const usageCell = (label: string, value: number | null, u: Usage) =>
  label === 'Browser time'
    ? duration(u.browser_seconds ?? 0)
    : label === 'Bytes out'
      ? bytes(u.bytes_out ?? 0)
      : num(value);

/** Throttling and error rows turn yellow once they are above zero. */
export const usageWarns = (label: string, value: number | null) =>
  /limited|denied|errors/.test(label) && (value ?? 0) > 0;
