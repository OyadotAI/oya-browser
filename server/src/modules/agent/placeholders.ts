/**
 * Task values in text: `{{key|filter}}` placeholders filled from values, and values
 * turned back into placeholders so the model and playbooks never hold them.
 */
import { MIN_REDACT_LENGTH } from './constants.ts';

/**
 * Transforms a placeholder applies to its value, so a run can split or reformat a task
 * value and still replay with other data: {{name|first}}, {{dob|date:MM/DD/YYYY}}.
 * Self-contained arrow functions: renderPlaywright() copies their source into the export.
 */
export const FILTERS = {
  first: (s) => s.trim().split(/\s+/)[0] || '',
  last: (s) => s.trim().split(/\s+/).at(-1) || '',
  part: (s, n) => s.trim().split(/\s+/)[Number(n) - 1] || '',
  upper: (s) => s.toUpperCase(),
  lower: (s) => s.toLowerCase(),
  digits: (s) => s.replace(/\D/g, ''),
  /* eslint-disable max-lines-per-function, no-magic-numbers -- copied verbatim into Playwright exports, so it stays self-contained */
  date: (s, format = 'MM/DD/YYYY') => {
    const iso = /^\d{4}-\d{2}-\d{2}$/.test(s.trim());
    const d = new Date(iso ? `${s.trim()}T00:00:00Z` : s);
    if (Number.isNaN(d.getTime())) return s;
    const [y, m, day] = iso
      ? [d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()]
      : [d.getFullYear(), d.getMonth(), d.getDate()];
    const months = [
      'January',
      'February',
      'March',
      'April',
      'May',
      'June',
      'July',
      'August',
      'September',
      'October',
      'November',
      'December',
    ];
    const pad = (x) => String(x).padStart(2, '0');
    const parts = {
      YYYY: String(y),
      YY: String(y).slice(-2),
      MMMM: months[m],
      MMM: months[m].slice(0, 3),
      MM: pad(m + 1),
      M: String(m + 1),
      DD: pad(day),
      D: String(day),
    };
    return format.replace(/YYYY|YY|MMMM|MMM|MM|M|DD|D/g, (t) => parts[t]);
  },
  /* eslint-enable max-lines-per-function, no-magic-numbers */
};

/** `{{key}}` or `{{key|filter|filter:arg}}`. */
export const PLACEHOLDER = /\{\{(\w+)((?:\|\w+(?::[^|}]*)?)*)\}\}/g;

/** "|first|date:MM/DD" -> [['first'], ['date', 'MM/DD']] */
export const pipesOf = (raw = '') =>
  raw
    .split('|')
    .slice(1)
    .map((p) => {
      const i = p.indexOf(':');
      return i < 0 ? [p] : [p.slice(0, i), p.slice(i + 1)];
    });

/** Placeholders filled from values with their filters applied; unknown keys stay as typed. */
export const fill = (text, data = {}) =>
  typeof text === 'string'
    ? text.replace(PLACEHOLDER, (m, k, raw) =>
        data[k] != null
          ? pipesOf(raw).reduce((s, [name, arg]) => (FILTERS[name] ? FILTERS[name](s, arg) : s), String(data[k]))
          : m,
      )
    : text;

/** Data values turned back into their placeholders, so the model never reads them. */
export function redact(text, data = {}) {
  if (typeof text !== 'string') return text;
  // ponytail: values under 3 characters are left alone; redacting them would blank unrelated text.
  const pairs = Object.entries(data)
    .map(([k, v]) => [k, String(v ?? '')])
    .filter(([, v]) => v.length >= MIN_REDACT_LENGTH)
    .sort((a, b) => b[1].length - a[1].length);
  for (const [k, v] of pairs) text = text.split(v).join(`{{${k}}}`);
  return text;
}

/** A file task value, as the SDK's file() builds it. */
export const isFileValue = (v) => !!v && typeof v === 'object' && typeof v.b64 === 'string';

/**
 * upload_file takes the name of a file, not a placeholder, but every other tool is
 * taught to write `{{name}}`, so the model reaches for one here too. Take both.
 */
export const dataKey = (v) => String(v ?? '').replace(/^\{\{\s*|\s*\}\}$/g, '');
