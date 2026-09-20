/**
 * MFA.
 *
 * An agent acting for someone on their own accounts hits second factors, and a
 * challenge nobody can answer is where automation stops. Four paths behind one
 * call: TOTP, email OTP, SMS OTP, and handing the session to a person.
 *
 * TOTP seeds are credential material of the same weight as a password. They are
 * encrypted at rest with the shared envelope scheme, never logged, and never
 * returned by the API — only whether one is configured.
 */

import { setTimeout as sleep } from 'timers/promises';
import { metrics } from '../../platform/metrics.ts';
import { totp } from './totp.ts';
import { load } from './mfa-factors.ts';
import { DETECT_JS, fillCodeJS, SUBMIT_CODE_JS } from './mfa-page.ts';
import { loading, readPage, whenReady } from './page-ready.ts';
import { fetchRelayCode } from './mfa-code.ts';
import { MFA_CONFIRM_MS, MFA_POLL_MS } from './constants.ts';

export { totp } from './totp.ts';
export { restore, TYPES, set, clear, clearAll, describe, list, reset } from './mfa-factors.ts';
export { DETECT_JS, fillCodeJS } from './mfa-page.ts';
export { extractCode, codeInReply } from './mfa-code.ts';

/**
 * Type a code into the challenge on the page and submit it.
 *
 * Split out of complete() because a code does not only come from a configured
 * factor: a person who answers the parked run by replying with the code has
 * answered the challenge, and making them open the live view to type it again
 * would be the automation wasting their time.
 */
export async function submitCode(evaluate, code, segmented = null) {
  // Detect first rather than trusting a caller's earlier pass: the relay poll
  // can take 90 seconds and a parked run can wait half an hour, and the marks
  // DETECT_JS leaves do not survive the navigation either one may have caused.
  const layout = segmented === null ? await detectLayout(evaluate) : segmented;
  if (layout === null) return { present: false, filled: false, submitted: false, completed: false };
  return typeCode(evaluate, code, layout);
}

/** Whether the code boxes are one per digit, or null when there is no code prompt to type into. */
async function detectLayout(evaluate) {
  const found = await whenReady(evaluate, DETECT_JS);
  if (!found?.present || found.handoff) return null;
  return !!found.segmented;
}

/** Fills, submits and waits for the challenge to go away. */
async function typeCode(evaluate, code, segmented) {
  const filled = await evaluate(fillCodeJS(code, segmented));
  if (!filled?.filled)
    return { present: true, filled: false, submitted: false, completed: false, reason: filled?.reason };
  const submitted = !!(await evaluate(SUBMIT_CODE_JS));
  // Filling an input is not proof the site accepted a factor. A disappearing
  // challenge after submission is the observable success signal.
  const completed = await challengeCleared(evaluate, Date.now() + (submitted ? MFA_CONFIRM_MS : 0));
  return { present: true, filled: true, submitted, completed };
}

/** Re-detects until the challenge is gone or the deadline passes. */
async function challengeCleared(evaluate, deadline) {
  do {
    const completed = await promptGone(evaluate);
    if (completed || Date.now() >= deadline) return completed;
    await sleep(MFA_POLL_MS);
  } while (true);
}

/** Whether the prompt has gone; a page that cannot be read (mid-navigation) has not confirmed anything yet. */
async function promptGone(evaluate) {
  const seen = await readPage(evaluate, DETECT_JS);
  // A page that has not loaded has not confirmed anything: reporting it gone is
  // how a code the site refused reported completion.
  return loading(seen) ? false : !seen?.present;
}

/**
 * Complete a challenge.
 *
 * @param evaluate  runs a script in the page
 * @param personaId whose factor to use
 * @param domain    the site being signed in to, so a persona can hold one
 *        factor per portal; falls back to the persona-wide factor
 * @param since     epoch ms of the login that asked for this code — anything
 *        older belongs to a previous run
 * @param llm       the tenant's own LLM, which reads the code out of the
 *        message; without one the pattern is used instead
 * @param liveViewUrl surfaced when nothing can answer it — a person finishing
 *        the challenge by hand is a real outcome, not a failure, and it is the
 *        only answer for push-approval factors.
 */
export async function complete(evaluate, personaId, { liveViewUrl = null, domain = null, since = 0, llm = null } = {}) {
  const found = await whenReady(evaluate, DETECT_JS);
  if (!found?.present) return { present: false, completed: false, method: 'none' };
  const config = load(personaId, domain);
  if (!config || found.handoff) return needsPerson(domain, liveViewUrl);
  const got = await codeFor(config, since, llm);
  if ('error' in got) return codeFailed(config, got.error, liveViewUrl);
  return enterCode(evaluate, found, config, got.code, liveViewUrl);
}

/** Nothing configured can answer this challenge: hand it to a person. */
function needsPerson(domain, liveViewUrl) {
  metrics.mfaCompleted.inc({ method: 'handoff', outcome: 'needed' });
  const scope = domain ? `${domain} or ` : '';
  const error = `No MFA factor is configured for ${scope}this persona. Open the live view to complete it by hand.`;
  return { present: true, completed: false, method: 'handoff', liveViewUrl, error };
}

/** The factor's current code, or the reason it could not be had. */
async function codeFor(config, since, llm) {
  try {
    return { code: config.type === 'totp' ? totp(config.secret) : await fetchRelayCode(config, since, llm) };
  } catch (err) {
    return { error: err.message };
  }
}

/** The code could not be obtained. */
function codeFailed(config, error, liveViewUrl) {
  metrics.mfaCompleted.inc({ method: config.type, outcome: 'error' });
  return { present: true, completed: false, method: config.type, liveViewUrl, error };
}

/** Types the code in and reports whether the site took it. */
async function enterCode(evaluate, found, config, code, liveViewUrl) {
  const { filled, submitted, completed } = await submitCode(evaluate, code, !!found.segmented);
  metrics.mfaCompleted.inc({ method: config.type, outcome: completed ? 'ok' : 'needs_attention' });
  const report = { present: true, completed, filled, submitted, method: config.type, segmented: !!found.segmented };
  return { ...report, ...(completed ? {} : { liveViewUrl, error: unconfirmed(filled) }) };
}

/** Why an entered code did not finish the challenge. */
function unconfirmed(filled) {
  return filled
    ? 'The code was entered, but the site has not confirmed it. Open the live view to finish.'
    : 'Could not fill the code field';
}
