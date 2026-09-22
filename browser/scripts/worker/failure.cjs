/**
 * Why a step failed, in words a person can act on: which target was tried,
 * and whether it was missing, ambiguous, hidden, covered, slow, or on the
 * wrong page. One message for every failure left people guessing.
 * The run redacts the result before anyone sees it.
 */
const { MESSAGE_MAX_CHARS } = require('../constants.cjs').FAILURE;

/** What a failure says when nothing more specific is known. */
const STEP_FAILED = 'The step did not complete. Check the matching target, timeout, and expected result.';

/** A step's first target as a person reads it: its kind and value, inside its frames. */
function targetOf(step) {
  const c = step?.candidates?.[0];
  if (!c) return '';
  const frames = step.frames?.length ? ` in frame ${step.frames.join(' → ')}` : '';
  const others = step.candidates.length - 1;
  const rest = others ? ` (and ${others} other recorded target${others === 1 ? '' : 's'})` : '';
  return `${c.kind === 'role' ? c.role : c.kind} "${c.value}"${frames}${rest}`;
}

/** The first line of Playwright's message, and the "Received" line of an assertion. */
function playwrightLines(message) {
  const lines = String(message)
    .split('\n')
    .map((line) => line.trim());
  const received = lines.find((line) => /^Received( string)?:/.test(line));
  return { first: lines[0].replace(/^Error: /, ''), received };
}

/** Pattern in the error → what to say, given the step and its target. */
const EXPLAIN = [
  [/elements match\. Pick a unique target|Target not found/, (e, t) => `${e.message} Tried ${t}.`],
  [/strict mode violation/, (e, t) => `More than one element matches ${t}. Pick a unique target.`],
  [/toHaveURL/, (e, t, lines) => `The page is not the expected one. ${lines.received || ''}`.trim()],
  [/expect\(/, (e, t, lines) => `Check failed on ${t || 'the page'}. ${lines.received || lines.first}`.trim()],
  [/intercepts pointer events/, (e, t) => `Something on the page covers ${t}, such as a dialog or a banner.`],
  [
    /not visible|element is hidden/,
    (e, t) => `${t} is on the page but hidden. A Hover or Scroll step before it may show it.`,
  ],
  [/Timeout \d+ms exceeded|TimeoutError/, (e, t) => `Timed out waiting for ${t || 'the page'}.`],
];

/** The message a person sees for a step's `error`. */
function failureMessage(error, step) {
  const message = String(error?.message || '');
  if (message === 'Run stopped') return 'Run stopped';
  const found = EXPLAIN.find(([pattern]) => pattern.test(message));
  if (!found) return STEP_FAILED;
  return found[1](error, targetOf(step), playwrightLines(message)).slice(0, MESSAGE_MAX_CHARS);
}

module.exports = { failureMessage, STEP_FAILED };
