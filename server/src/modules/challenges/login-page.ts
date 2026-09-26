/**
 * The page side of signing in: scripts that run in the browser to find a login
 * form, fill it, and ask the site for a code.
 */

/**
 * Find the sign-in form and read what filling it needs. Runs in the page.
 * Deliberately read-only, it identifies, it does not interact.
 */
export const DETECT_JS = `(() => {
  // Mid-navigation this document is empty, and "no form here" would read as a
  // verdict. Saying so instead is what lets the caller wait.
  if (document.readyState !== 'complete') return { present: false, loading: true };
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
    // login: some portals put one between the password and the code.
    //
    // Only on a page that is plainly about verifying, though. "Send email" is
    // also what a contact form and a footer say, and clicking one of those
    // mid-run would be the automation doing something nobody asked for.
    const verifying = /\\b(verification|verify|one.?time|security code|authentication code|two.?factor|2fa|mfa|passcode)\\b/i.test(text)
      || /\\bcheck your (email|inbox|phone|messages)\\b|\\bwe (just )?(sent|emailed|texted)\\b|\\b(a|the|your) code\\b/i.test(text);
    const request = !verifying ? null : [...document.querySelectorAll('button, input[type="submit"], input[type="button"], a')].find((b) =>
      visible(b) && /^(send|resend|request)( me)?( an?| the)? (email|code|sms|text|one.?time|verification)|^(send|resend) (email|code)$/i.test((b.innerText || b.value || '').trim()));
    if (request) { request.setAttribute('data-oya-login-target', 'request'); return { present: true, stage: 'request_code', rejected, locked }; }

    // Microsoft, Okta and Google ask for the account on its own page and only
    // then for the password. With no password box to anchor on, the signal is an
    // identifier field with a Next-shaped control: a search box next to a
    // "Continue" link is not a sign-in step, so both must be true.
    //
    // And the page has to be about signing in: its text or title says so, or the
    // field declares itself a username (autocomplete="username", which no search
    // box does). Carelon's "User Confirmation" page says neither "sign in" nor
    // "log in" anywhere in its body, only in its title and its field.
    const named = (el) => [el.name, el.id, el.autocomplete, el.placeholder, el.getAttribute('aria-label')].filter(Boolean).join(' ');
    const identifier = [...document.querySelectorAll('input')].find((el) => visible(el)
      && /^(text|email)$/.test(el.type || 'text')
      && /\\b(user.?name|user.?id|email|account|login|identifier)\\b/i.test(named(el)));
    const next = !identifier ? null : [...(identifier.form || document).querySelectorAll('button, input[type="submit"]')].find((b) =>
      visible(b) && !b.disabled && /^(next|continue|sign ?in|log ?in|submit)$/i.test((b.innerText || b.value || '').trim()));
    const aboutSignIn = /\\b(sign ?in|log ?in|signin)\\b/i.test(text + ' ' + document.title)
      || /\\busername\\b/i.test((identifier && identifier.autocomplete) || '');
    if (identifier && next && aboutSignIn) {
      identifier.setAttribute('data-oya-login-target', 'username');
      next.setAttribute('data-oya-login-target', 'next');
      return { present: true, stage: 'username', rejected, locked };
    }
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

  const controls = [...(form || document).querySelectorAll('button, input[type="submit"]')].filter((b) =>
    visible(b) && !b.disabled && !/^(cancel|reset|forgot|register|sign ?up)/i.test((b.innerText || b.value || '').trim()));
  // A password reveal toggle is a type="button" with no text, and portals put
  // it inside the form before the real control. Clicking it flips the field to
  // type="text", which the next detect reads as the form having gone away, a
  // sign-in that reports success having never submitted, with the password now
  // on screen. Anything that actually posts a form is a submit.
  const submit = controls.find((b) => b.type === 'submit') || controls[0];
  if (submit) submit.setAttribute('data-oya-login-target', 'submit');

  return { present: true, stage: 'credentials', hasUsername: !!username, hasSubmit: !!submit, rejected, locked };
})()`;

/** Form filler, up to the credentials. */
const FILL_CREDENTIALS_HEAD = `(() => {
  const values = `;
/** Form filler, after the credentials. */
const FILL_CREDENTIALS_TAIL = `;
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

/**
 * Fill and submit. The native value setter plus input/change is the same
 * technique mfa.js uses, and for the same reason: a framework-controlled input
 * ignores a plain assignment, so the page would submit an empty form.
 */
export const fillCredentialsJS = (username, password) =>
  `${FILL_CREDENTIALS_HEAD}${JSON.stringify({ username: String(username), password: String(password) })}${FILL_CREDENTIALS_TAIL}`;

/** Ask the site to send the code. Its own step on portals that have one. */
export const requestCodeJS = `(() => {
  const el = document.querySelector('[data-oya-login-target="request"]');
  if (!el) return { requested: false, reason: 'no request control' };
  el.click();
  return { requested: true };
})()`;

/** Sets an input the way a person would, so the page's own listeners run. */
const FIRE_VALUE = `(el, v) => {
  el.focus();
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, v);
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
}`;

/**
 * Gives the account name on an account-first page and presses Next. Separate
 * from the credentials filler because there is no password to type yet, and
 * clicking Next is what makes the password page exist.
 */
export const fillUsernameJS = (username) => `(() => {
  const fire = ${FIRE_VALUE};
  const el = document.querySelector('[data-oya-login-target="username"]');
  if (!el) return { filled: false, reason: 'account field not found' };
  fire(el, ${JSON.stringify(username)});
  const next = document.querySelector('[data-oya-login-target="next"]');
  if (next) next.click();
  return { filled: true, submitted: !!next };
})()`;
