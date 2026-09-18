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

import * as credentials from './credentials.js';
import { metrics } from './metrics.js';

/**
 * Find the sign-in form and read what filling it needs. Runs in the page.
 * Deliberately read-only — it identifies, it does not interact.
 */
export const DETECT_JS = `(() => {
  const visible = (el) => !(el.type === 'hidden' || el.disabled || el.readOnly || !el.getClientRects().length || getComputedStyle(el).visibility === 'hidden');
  const text = (document.body && document.body.innerText || '');

  document.querySelectorAll('[data-oya-login-target]').forEach((el) => el.removeAttribute('data-oya-login-target'));

  // A rejected password and a lockout are the two states that must never be
  // retried into. Read them before anything else: on these portals the form is
  // still on the page underneath the error.
  const rejected = /(invalid|incorrect|wrong|not recognou?ized|not recognised|do(es)? not match|unable to (log|sign) ?in|authentication failed)[^.]{0,40}(username|user ?id|password|credential|login)|(username|user ?id|password|credential)[^.]{0,40}(invalid|incorrect|not recognou?ized|not recognised)/i.test(text);
  const locked = /(account|profile)[^.]{0,30}(locked|disabled|suspended)|too many (failed )?(login |sign.?in )?attempts/i.test(text);

  const password = [...document.querySelectorAll('input[type="password"]')].find(visible);
  if (!password) {
    // No password box. A request-a-code button on its own is still part of the
    // login: Carelon and friends put one between the password and the code.
    //
    // Only on a page that is plainly about verifying, though. "Send email" is
    // also what a contact form and a footer say, and clicking one of those
    // mid-run would be the automation doing something nobody asked for.
    const verifying = /\\b(verification|verify|one.?time|security code|authentication code|two.?factor|2fa|mfa|passcode)\\b/i.test(text)
      || /\\bcheck your (email|inbox|phone|messages)\\b|\\bwe (just )?(sent|emailed|texted)\\b|\\b(a|the|your) code\\b/i.test(text);
    const request = !verifying ? null : [...document.querySelectorAll('button, input[type="submit"], input[type="button"], a')].find((b) =>
      visible(b) && /^(send|resend|request)( me)?( an?| the)? (email|code|sms|text|one.?time|verification)|^(send|resend) (email|code)$/i.test((b.innerText || b.value || '').trim()));
    if (request) { request.setAttribute('data-oya-login-target', 'request'); return { present: true, stage: 'request_code', rejected, locked }; }
    return { present: false, rejected, locked };
  }
  password.setAttribute('data-oya-login-target', 'password');

  // The username is the visible text-ish input before the password in the same
  // form, which is what these forms look like and what survives renamed ids.
  const form = password.form;
  const candidates = [...(form || document).querySelectorAll('input')].filter((el) =>
    visible(el) && /^(text|email|tel)$/.test(el.type || 'text'));
  const before = candidates.filter((el) => password.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_PRECEDING);
  const username = before[before.length - 1] || candidates[0] || null;
  if (username) username.setAttribute('data-oya-login-target', 'username');

  const submit = [...(form || document).querySelectorAll('button, input[type="submit"]')].find((b) =>
    visible(b) && !b.disabled && !/^(cancel|reset|forgot|register|sign ?up)/i.test((b.innerText || b.value || '').trim()));
  if (submit) submit.setAttribute('data-oya-login-target', 'submit');

  return { present: true, stage: 'credentials', hasUsername: !!username, hasSubmit: !!submit, rejected, locked };
})()`;

/**
 * Fill and submit. The native value setter plus input/change is the same
 * technique mfa.js uses, and for the same reason: a framework-controlled input
 * ignores a plain assignment, so the page would submit an empty form.
 */
export const fillCredentialsJS = (username, password) => `(() => {
  const values = ${JSON.stringify({ username: String(username), password: String(password) })};
  const fire = (el, v) => {
    el.focus();
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };
  const passwordEl = document.querySelector('[data-oya-login-target="password"]');
  if (!passwordEl) return { filled: false, reason: 'password field not found' };
  const usernameEl = document.querySelector('[data-oya-login-target="username"]');
  if (usernameEl) fire(usernameEl, values.username);
  fire(passwordEl, values.password);

  const submit = document.querySelector('[data-oya-login-target="submit"]');
  if (submit) { submit.click(); return { filled: true, submitted: true }; }
  if (passwordEl.form) { passwordEl.form.requestSubmit(); return { filled: true, submitted: true }; }
  return { filled: true, submitted: false };
})()`;

/** Ask the site to send the code. Its own step on portals that have one. */
export const requestCodeJS = `(() => {
  const el = document.querySelector('[data-oya-login-target="request"]');
  if (!el) return { requested: false, reason: 'no request control' };
  el.click();
  return { requested: true };
})()`;

/**
 * How many times this browser has tried a given site, so a wrong password
 * cannot be typed until the account locks.
 *
 * ponytail: in memory, cleared when the browser stops; a restart forgets and
 * allows the attempts again. Move it beside the run if that stops being true.
 */
const attempts = new Map(); // `${browserId}|${domain}` -> count
const MAX_ATTEMPTS = 2;

export const attemptsFor = (browserId, domain) => attempts.get(`${browserId}|${domain}`) || 0;
export function forget(browserId, domain = null) {
  if (domain) { attempts.delete(`${browserId}|${domain}`); return; }
  for (const key of [...attempts.keys()]) if (key.startsWith(`${browserId}|`)) attempts.delete(key);
}

/**
 * Sign in, if this is a login page and we hold credentials for it.
 *
 * @param evaluate   runs a script in the page
 * @param personaId  whose credentials to use
 * @param domain     the site, as credentials.domainOf() files it
 * @param browserId  scopes the attempt counter
 * @returns the code was requested / credentials were accepted / a person is needed
 */
export async function complete(evaluate, personaId, { domain, browserId, liveViewUrl = null } = {}) {
  const found = await evaluate(DETECT_JS);
  if (!found?.present) return { present: false, completed: false, method: 'none', locked: !!found?.locked };

  // A locked account is never fixed by trying again, and the next attempt is
  // what turns a lockout into a support ticket.
  if (found.locked) {
    metrics.loginCompleted.inc({ method: 'credentials', outcome: 'locked' });
    return {
      present: true, completed: false, method: 'handoff', locked: true, liveViewUrl,
      error: 'The site says this account is locked. Automation stopped rather than trying again.',
    };
  }

  if (found.stage === 'request_code') {
    const asked = await evaluate(requestCodeJS);
    return { present: true, completed: !!asked?.requested, method: 'request_code', requestedAt: Date.now(), liveViewUrl,
      ...(asked?.requested ? {} : { error: asked?.reason || 'Could not ask the site for a code' }) };
  }

  const stored = domain ? credentials.lookup(personaId, domain) : null;
  if (!stored) {
    metrics.loginCompleted.inc({ method: 'handoff', outcome: 'needed' });
    return {
      present: true, completed: false, method: 'handoff', liveViewUrl,
      error: `No credentials are stored for ${domain || 'this site'}. Sign in in the live view, or add them to the persona.`,
    };
  }

  // A password the site has just refused is not a flaky fill. Retrying it can
  // only spend one more of the portal's attempts, so this stops here — the
  // retry budget below exists for a submit the page swallowed, nothing else.
  if (found.rejected) {
    metrics.loginCompleted.inc({ method: 'credentials', outcome: 'rejected' });
    return {
      present: true, completed: false, method: 'credentials', rejected: true, liveViewUrl,
      error: `The site rejected the stored credentials for ${stored.username} at ${stored.domain}. `
        + 'They were not tried again. Correct them on the persona, or sign in in the live view.',
    };
  }

  const key = `${browserId}|${stored.domain}`;
  const used = attempts.get(key) || 0;
  if (used >= MAX_ATTEMPTS) {
    metrics.loginCompleted.inc({ method: 'credentials', outcome: 'exhausted' });
    return {
      present: true, completed: false, method: 'credentials', exhausted: true, liveViewUrl,
      error: `Signing in to ${stored.domain} did not take after ${used} attempts. Stopped before the account could lock.`,
    };
  }
  attempts.set(key, used + 1);

  const filled = await evaluate(fillCredentialsJS(stored.username, stored.password));
  if (!filled?.filled) {
    metrics.loginCompleted.inc({ method: 'credentials', outcome: 'error' });
    return { present: true, completed: false, method: 'credentials', liveViewUrl, error: filled?.reason || 'Could not fill the sign-in form' };
  }

  // Filling a form is not proof the site accepted it. The observable signal is
  // the password box going away — or an error appearing where it stood.
  const submittedAt = Date.now();
  const deadline = submittedAt + (filled.submitted ? 20_000 : 0);
  let after = found;
  do {
    try { after = await evaluate(DETECT_JS); } catch { after = { present: false }; }
    if (!after?.present || after.rejected || after.locked || Date.now() >= deadline) break;
    await new Promise((r) => setTimeout(r, 500));
  } while (true);

  const completed = !after?.present || after.stage === 'request_code';
  if (completed) { attempts.delete(key); }
  metrics.loginCompleted.inc({ method: 'credentials', outcome: completed ? 'ok' : 'needs_attention' });
  return {
    present: true, completed, method: 'credentials', submitted: !!filled.submitted, submittedAt,
    username: stored.username, domain: stored.domain,
    rejected: !!after?.rejected, locked: !!after?.locked,
    ...(completed ? {} : {
      liveViewUrl,
      error: after?.rejected
        ? `The site rejected the stored credentials for ${stored.username} at ${stored.domain}.`
        : after?.locked
          ? 'The site says this account is now locked.'
          : 'The sign-in form is still showing. Open the live view to finish.',
    }),
  };
}
