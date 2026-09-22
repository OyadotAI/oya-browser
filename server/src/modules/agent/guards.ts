/**
 * What keeps a run from wasting itself: a loop of calls that change nothing, an
 * empty reply, and a final answer that does not say how it went.
 *
 * A call is the same as an earlier one when its name, its arguments and what it
 * returned all match, so scrolling a long list (the page reads differently each
 * time) is progress, while clicking a button that does nothing is not.
 */
import { createHash } from 'node:crypto';
import {
  CYCLE_LENGTHS,
  CYCLE_REPEATS,
  EMPTY_REPLIES_ALLOWED,
  GUARD_HISTORY,
  REPEAT_WARNING_AFTER,
  RESULT_DIGEST_CHARS,
  STUCK_NOTES_ALLOWED,
} from './constants.ts';

/** A run's memory of its recent calls and of the warnings it has been given. */
export type Guard = {
  /** How many calls the run has made, kept through the cycle warnings that clear `calls`. */
  total: number;
  /** The last calls, as keys (name, arguments and a digest of the result). */
  calls: string[];
  /** How many times the run was told it is going round in circles. */
  notes: number;
  /** How many empty replies in a row the model gave. */
  empty: number;
};

/** A fresh guard for a run. */
export const newGuard = (): Guard => ({ total: 0, calls: [], notes: 0, empty: 0 });

/** One call as a key: two calls are the same when they asked the same thing and got the same answer. */
function keyOf(name: string, args: unknown, result: string) {
  const digest = createHash('sha256').update(String(result)).digest('hex').slice(0, RESULT_DIGEST_CHARS);
  return `${name}${JSON.stringify(args)}#${digest}`;
}

/** Whether the last calls repeat a cycle of `length`: three identical calls, or a longer cycle twice over. */
function repeats(calls: string[], length: number) {
  const times = length === 1 ? REPEAT_WARNING_AFTER : CYCLE_REPEATS;
  if (calls.length < length * times) return false;
  const tail = calls.slice(-length * times);
  return tail.every((key, i) => key === tail[i % length]);
}

/** The shortest cycle the last calls are stuck in, or 0 when they are not. */
const cycleOf = (calls: string[]) => CYCLE_LENGTHS.find((length) => repeats(calls, length)) || 0;

/** What the model is told when it is going round in circles. */
const circling = (length: number) =>
  length === 1
    ? 'NOTE: this is the same call with the same result several times in a row, so it is not working.'
    : `NOTE: your last ${length * CYCLE_REPEATS} calls repeat the same ${length} steps and the page has not changed.`;

/**
 * Remembers a call and answers the note to add to its result: none, a warning, or,
 * after STUCK_NOTES_ALLOWED warnings, `stop` with the report the run ends on.
 */
export function afterCall(guard: Guard, name: string, args: unknown, result: string) {
  remember(guard, keyOf(name, args, result));
  const length = cycleOf(guard.calls);
  if (!length) return { note: '' };
  if (++guard.notes > STUCK_NOTES_ALLOWED)
    return { note: '', stop: `FAILED: the agent kept repeating ${name} without the page changing.` };
  guard.calls.length = 0;
  return { note: `\n\n${circling(length)} ${TRY_ELSE}` };
}

/** What a warned model is asked to do instead. */
const TRY_ELSE =
  'Try something different (another element, another value format, scrolling, or analyze_page), or stop and explain what is blocking you.';

/** Adds a call to the history, keeping the last GUARD_HISTORY; a call also ends a run of empty replies. */
function remember(guard: Guard, key: string) {
  guard.total++;
  guard.calls.push(key);
  guard.calls.splice(0, Math.max(0, guard.calls.length - GUARD_HISTORY));
  guard.empty = 0;
}

/** What an empty reply gets: a nudge to act or finish, or, when it keeps happening, the report the run ends on. */
export function afterEmpty(guard: Guard) {
  if (++guard.empty > EMPTY_REPLIES_ALLOWED) return { stop: 'FAILED: the model stopped answering.' };
  return {
    nudge:
      'You replied with nothing. Call a tool to continue, or finish with a report whose first line starts with DONE: or FAILED:.',
  };
}

/** Whether a final answer reports a failure: a line of it starts with FAILED:, as the prompt asks the report to. */
export const failed = (text: string) => /(^|\n)\s*FAILED:/i.test(text);
