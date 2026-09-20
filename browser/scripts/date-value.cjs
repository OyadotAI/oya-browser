/**
 * The value a native date or time input takes, from the text an agent or a
 * playbook meant to type. Typing into these inputs goes wrong: they are split
 * into locale-ordered segments (mm/dd/yyyy in the US), cannot be selected and
 * cleared, and each digit lands in whichever segment has focus. Their value,
 * though, is always one machine format (YYYY-MM-DD for a date), so both
 * drivers set it directly, the way the browser's own picker does. Shared by the
 * desktop driver (main/actions) and the server's CDP driver.
 */
const { DATES } = require('./constants.cjs');

/** A date written year first: 2024-01-15, 2024-1-5, 2024-01, with an optional time. */
const ISO_DATE = /^(\d{4})-(\d{1,2})(?:-(\d{1,2}))?/;

/** A date written with numbers only: 01/15/2024, 15.01.2024, 1-15-24. */
const NUMERIC_DATE = /^(\d{1,2})[/.-](\d{1,2})[/.-](\d{4}|\d{2})\b/;

/** A time of day anywhere in the text: 15:30, 3:30 PM, 03:30:00 a.m. */
const CLOCK = /(\d{1,2}):(\d{2})(?::\d{2})?\s*([ap])?\.?m?\.?/i;

/** An ISO week, the one format a week input takes: 2024-W03. */
const ISO_WEEK = /^\d{4}-W\d{2}$/;

/** How each input type's value is written, for the error that names it. */
const EXAMPLES = {
  date: 'YYYY-MM-DD',
  'datetime-local': 'YYYY-MM-DD HH:mm',
  month: 'YYYY-MM',
  week: 'YYYY-Www',
  time: 'HH:mm',
};

/** `n` with leading zeros to `width` digits. */
const pad = (n, width = DATES.PART_WIDTH) => String(n).padStart(width, '0');

/** The year, month and day of a year-first date. A month alone means its first day. */
function isoParts(text) {
  const match = ISO_DATE.exec(text);
  return match && { year: Number(match[1]), month: Number(match[2]), day: Number(match[3] || 1) };
}

/** The parts of a numbers-only date: month first, as US forms write it, unless the first number cannot be a month. */
function numericParts(text) {
  const match = NUMERIC_DATE.exec(text);
  if (!match) return null;
  const [first, second] = [Number(match[1]), Number(match[2])];
  const year = match[3].length === DATES.PART_WIDTH ? DATES.CENTURY + Number(match[3]) : Number(match[3]);
  return first > DATES.MONTHS ? { year, month: second, day: first } : { year, month: first, day: second };
}

/** The parts of a date written in words ("Jan 15, 2024"), read in local time. Text without letters is left to the patterns above. */
function writtenParts(text) {
  const time = /[a-z]/i.test(text) ? Date.parse(text) : NaN;
  if (Number.isNaN(time)) return null;
  const date = new Date(time);
  return { year: date.getFullYear(), month: date.getMonth() + 1, day: date.getDate() };
}

/** Whether the parts name a day that exists (no 31 April, no 29 February in 2023). */
function realDay({ year, month, day }) {
  if (month < 1 || month > DATES.MONTHS || day < 1 || day > DATES.MAX_DAY) return false;
  return new Date(Date.UTC(year, month - 1, day)).getUTCDate() === day;
}

/** The day `text` names, or null when it names none. */
function dayOf(text) {
  const parts = isoParts(text) || numericParts(text) || writtenParts(text);
  return parts && realDay(parts) ? parts : null;
}

/** The hour and minute in `text` on a 24-hour clock, or null. */
function clockOf(text) {
  const match = CLOCK.exec(text);
  if (!match) return null;
  const [hour, minute, half] = [Number(match[1]), Number(match[2]), (match[3] || '').toLowerCase()];
  const hours = half ? (hour % DATES.NOON) + (half === 'p' ? DATES.NOON : 0) : hour;
  return hours < DATES.HOURS && minute < DATES.MINUTES ? { hours, minute } : null;
}

/** YYYY-MM-DD. */
const dateString = (p) => `${pad(p.year, DATES.YEAR_WIDTH)}-${pad(p.month)}-${pad(p.day)}`;

/** HH:mm. */
const timeString = (c) => `${pad(c.hours)}:${pad(c.minute)}`;

/** A date and time; a date without a time means its midnight. */
function dateTimeString(text) {
  const day = dayOf(text);
  return day && `${dateString(day)}T${timeString(clockOf(text) || { hours: 0, minute: 0 })}`;
}

/** Each input type's value from text, or null when the text names no such value. */
const FORMATS = {
  date: (text) => (dayOf(text) ? dateString(dayOf(text)) : null),
  month: (text) => (dayOf(text) ? dateString(dayOf(text)).slice(0, EXAMPLES.month.length) : null),
  'datetime-local': dateTimeString,
  time: (text) => (clockOf(text) ? timeString(clockOf(text)) : null),
  week: (text) => (ISO_WEEK.test(text) ? text : null),
};

/** Whether an input of this type takes a fixed-format date or time rather than typed text. */
const isDateInput = (type) => Object.hasOwn(FORMATS, String(type).toLowerCase());

/** The value an input of `type` takes for `text`, or null when `text` cannot be read as one. */
function dateInputValue(type, text) {
  const kind = String(type).toLowerCase();
  return Object.hasOwn(FORMATS, kind) ? FORMATS[kind](String(text).trim()) : null;
}

/** What the agent is told when its text is not a date the input can take. */
const unreadableDate = (type, text) =>
  `Could not read "${text}" as a ${type} value. Type it as ${EXAMPLES[String(type).toLowerCase()] || 'the field shows'}.`;

module.exports = { isDateInput, dateInputValue, unreadableDate };
