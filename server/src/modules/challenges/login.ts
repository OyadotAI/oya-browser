/**
 * Signing in to a site with stored credentials.
 *
 * The cookie jar is still the first answer — a persona that is already signed in
 * never reaches this code, because a login page is only ever *seen* when the
 * stored session is dead. This is the fallback for portals that expire sessions
 * server-side between runs, where an unattended run would otherwise stop at the
 * front door with nothing to recover with.
 *
 * Same shape as captcha.js and mfa.js: a read-only detector, a fill script, and
 * one complete() that always reports which path ran. A silent failure that
 * leaves an agent looping on a login form is worse than a clear "not signed in".
 */

import { setTimeout as sleep } from 'timers/promises';
import * as credentials from '../personas/credentials.ts';
import { metrics } from '../../platform/metrics.ts';
import { DETECT_JS, fillCredentialsJS, requestCodeJS } from './login-page.ts';
import { LOGIN_CONFIRM_MS, LOGIN_POLL_MS, MAX_LOGIN_ATTEMPTS } from './constants.ts';

export { DETECT_JS, fillCredentialsJS, requestCodeJS } from './login-page.ts';

/**
 * How many times this browser has tried a given site, so a wrong password
 * cannot be typed until the account locks.
 *
 * ponytail: in memory, cleared when the browser stops; a restart forgets and
 * allows the attempts again. Move it beside the run if that stops being true.
 */
const attempts = new Map(); // `${browserId}|${domain}` -> count

/** Sign-in attempts this browser has made on `domain`. */
export const attemptsFor = (browserId, domain) => attempts.get(`${browserId}|${domain}`) || 0;
/** Reset the attempt counter for one site, or for every site when `domain` is omitted. */
export function forget(browserId, domain = null) {
  if (domain) {
    attempts.delete(`${browserId}|${domain}`);
    return;
  }
  for (const key of [...attempts.keys()]) if (key.startsWith(`${browserId}|`)) attempts.delete(key);
}

/** One sign-in in progress: the stored login, its attempt-counter key and the live view to hand off to. */
type SignIn = {
  /** Runs a script in the page. */
  evaluate: (script: string) => Promise<any>;
  /** The stored credentials being used. */
  stored: any;
  /** `${browserId}|${domain}`, the attempt counter's key. */
  key: string;
  /** Where a person can finish by hand. */
  liveViewUrl: string | null;
};

/**
 * Sign in, if this is a login page and we hold credentials for it.
 *
 * @param evaluate   runs a script in the page
 * @param personaId  whose credentials to use
 * @param domain     the site, as credentials.domainOf() files it
 * @param browserId  scopes the attempt counter
 * @returns the code was requested / credentials were accepted / a person is needed
 */
export async function complete(evaluate, personaId, { domain, browserId, liveViewUrl = null }: any = {}) {
  const found = await evaluate(DETECT_JS);
  if (!found?.present) return { present: false, completed: false, method: 'none', locked: !!found?.locked };
  if (found.locked) return lockedOut(liveViewUrl);
  if (found.stage === 'request_code') return requestCode(evaluate, liveViewUrl);
  const stored = domain ? credentials.lookup(personaId, domain) : null;
  if (!stored) return noCredentials(domain, liveViewUrl);
  if (found.rejected) return rejectedAlready(stored, liveViewUrl);
  return signIn({ evaluate, stored, key: `${browserId}|${stored.domain}`, liveViewUrl });
}

/** A sign-in that stopped short: counted, and reported with the live view to finish by hand. */
function stopped(metric, fields, liveViewUrl, error) {
  metrics.loginCompleted.inc(metric);
  return { present: true, completed: false, ...fields, liveViewUrl, error };
}

/**
 * A locked account is never fixed by trying again, and the next attempt is
 * what turns a lockout into a support ticket.
 */
function lockedOut(liveViewUrl) {
  return stopped(
    { method: 'credentials', outcome: 'locked' },
    { method: 'handoff', locked: true },
    liveViewUrl,
    'The site says this account is locked. Automation stopped rather than trying again.',
  );
}

/** Presses the site's request-a-code control. */
async function requestCode(evaluate, liveViewUrl) {
  const asked = await evaluate(requestCodeJS);
  const head = { present: true, completed: !!asked?.requested, method: 'request_code', requestedAt: Date.now() };
  const failure = asked?.requested ? {} : { error: asked?.reason || 'Could not ask the site for a code' };
  return { ...head, liveViewUrl, ...failure };
}

/** Nothing stored for this site: a person has to sign in. */
function noCredentials(domain, liveViewUrl) {
  return stopped(
    { method: 'handoff', outcome: 'needed' },
    { method: 'handoff' },
    liveViewUrl,
    `No credentials are stored for ${domain || 'this site'}. Sign in in the live view, or add them to the persona.`,
  );
}

/**
 * A password the site has just refused is not a flaky fill. Retrying it can
 * only spend one more of the portal's attempts, so this stops here — the
 * retry budget exists for a submit the page swallowed, nothing else.
 */
function rejectedAlready(stored, liveViewUrl) {
  return stopped(
    { method: 'credentials', outcome: 'rejected' },
    { method: 'credentials', rejected: true },
    liveViewUrl,
    `The site rejected the stored credentials for ${stored.username} at ${stored.domain}. ` +
      'They were not tried again. Correct them on the persona, or sign in in the live view.',
  );
}

/** Spends one attempt: fills the form, waits for the site's verdict and reports it. */
async function signIn(run: SignIn) {
  const used = attempts.get(run.key) || 0;
  if (used >= MAX_LOGIN_ATTEMPTS) return exhausted(run, used);
  attempts.set(run.key, used + 1);
  const filled = await run.evaluate(fillCredentialsJS(run.stored.username, run.stored.password));
  if (!filled?.filled) return fillFailed(filled, run.liveViewUrl);
  // Filling a form is not proof the site accepted it. The observable signal is
  // the password box going away — or an error appearing where it stood.
  const submittedAt = Date.now();
  const after = await verdict(run.evaluate, submittedAt + (filled.submitted ? LOGIN_CONFIRM_MS : 0));
  return settle(after, !!filled.submitted, submittedAt, run);
}

/** The retry budget is spent: stop before the account can lock. */
function exhausted(run: SignIn, used) {
  return stopped(
    { method: 'credentials', outcome: 'exhausted' },
    { method: 'credentials', exhausted: true },
    run.liveViewUrl,
    `Signing in to ${run.stored.domain} did not take after ${used} attempts. Stopped before the account could lock.`,
  );
}

/** The form could not be filled. */
function fillFailed(filled, liveViewUrl) {
  return stopped(
    { method: 'credentials', outcome: 'error' },
    { method: 'credentials' },
    liveViewUrl,
    filled?.reason || 'Could not fill the sign-in form',
  );
}

/** Re-detects until the form is gone, the site answers with an error, or the deadline passes. */
async function verdict(evaluate, deadline) {
  do {
    const after = await detectOrGone(evaluate);
    if (!after?.present || after.rejected || after.locked || Date.now() >= deadline) return after;
    await sleep(LOGIN_POLL_MS);
  } while (true);
}

/** The detector's view of the page; a page that cannot be read (mid-navigation) counts as gone. */
async function detectOrGone(evaluate) {
  try {
    return await evaluate(DETECT_JS);
  } catch {
    return { present: false };
  }
}

/** Reports how the submitted sign-in ended, and clears the attempt counter on success. */
function settle(after, submitted, submittedAt, run: SignIn) {
  const completed = !after?.present || after.stage === 'request_code';
  if (completed) attempts.delete(run.key);
  metrics.loginCompleted.inc({ method: 'credentials', outcome: completed ? 'ok' : 'needs_attention' });
  const { username, domain } = run.stored;
  const report = { present: true, completed, method: 'credentials', submitted, submittedAt, username, domain };
  const flags = { rejected: !!after?.rejected, locked: !!after?.locked };
  return { ...report, ...flags, ...(completed ? {} : { liveViewUrl: run.liveViewUrl, error: unfinished(after, run) }) };
}

/** Why a submitted sign-in did not finish. */
function unfinished(after, run: SignIn) {
  if (after?.rejected)
    return `The site rejected the stored credentials for ${run.stored.username} at ${run.stored.domain}.`;
  if (after?.locked) return 'The site says this account is now locked.';
  return 'The sign-in form is still showing. Open the live view to finish.';
}
