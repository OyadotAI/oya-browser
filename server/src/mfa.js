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

import { createHmac } from 'crypto';
import { sealText, openText } from './secrets.js';
import * as inbox from './inbox.js';
import { chatCompletion } from './llm.js';
import { assertSafeTarget } from './net-guard.js';
import { metrics } from './metrics.js';
import { readFileSync, writeFileSync, mkdirSync, renameSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

/**
 * `personaId|domain` -> sealed config, or bare `personaId` for the persona-wide
 * default. One persona drives several portals and they do not agree on a factor
 * — an authenticator app here, an emailed code there — so the factor is filed
 * per site, and the persona-wide record is the fallback (and what every
 * mfa.json written before this keying was introduced still is).
 */
const configs = new Map();
const STORE = join(process.env.OYA_DATA_DIR || join(dirname(fileURLToPath(import.meta.url)), '..', 'data'), 'mfa.json');
export function restore() {
  try { for (const [id, value] of Object.entries(JSON.parse(readFileSync(STORE, 'utf8')))) configs.set(id, value); }
  catch (e) { if (e.code !== 'ENOENT') throw new Error(`Cannot read MFA settings: ${e.message}`); }
}
function persist() {
  mkdirSync(dirname(STORE), { recursive: true, mode: 0o700 });
  const temp = `${STORE}.${process.pid}.tmp`;
  writeFileSync(temp, JSON.stringify(Object.fromEntries(configs)), { mode: 0o600 });
  renameSync(temp, STORE);
}
restore();

const keyFor = (personaId, domain) => (domain ? `${personaId}|${domain}` : String(personaId));
const scopeFor = (key) => `mfa:${key}`;

// ── TOTP (RFC 6238) ──

function base32Decode(input) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  const clean = String(input).toUpperCase().replace(/[\s=-]/g, '');
  let bits = 0, value = 0;
  const out = [];
  for (const ch of clean) {
    const idx = alphabet.indexOf(ch);
    if (idx === -1) throw Object.assign(new Error('TOTP secret is not valid base32'), { status: 400 });
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) { out.push((value >>> (bits - 8)) & 0xff); bits -= 8; }
  }
  if (!out.length) throw Object.assign(new Error('TOTP secret is empty'), { status: 400 });
  return Buffer.from(out);
}

/**
 * @param {string} secret base32, as printed under a QR code
 * @param {number} [at] unix seconds, for testing against known vectors
 */
export function totp(secret, at = Math.floor(Date.now() / 1000), { digits = 6, period = 30, algorithm = 'sha1' } = {}) {
  const counter = Math.floor(at / period);
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64BE(BigInt(counter));
  const hmac = createHmac(algorithm, base32Decode(secret)).update(buf).digest();
  const offset = hmac[hmac.length - 1] & 0x0f;
  const code = ((hmac[offset] & 0x7f) << 24 | hmac[offset + 1] << 16 | hmac[offset + 2] << 8 | hmac[offset + 3])
    % 10 ** digits;
  return String(code).padStart(digits, '0');
}

// ── Configuration ──

export const TYPES = ['totp', 'email', 'sms', 'gmail', 'graph'];

export async function set(personaId, config, domain = null) {
  const { type } = config || {};
  if (!TYPES.includes(type)) {
    throw Object.assign(new Error(`mfa type must be one of ${TYPES.join(', ')}`), { status: 400 });
  }
  if (type === 'totp') {
    if (!config.secret) throw Object.assign(new Error('a TOTP secret is required'), { status: 400 });
    totp(config.secret);            // fail now, not at the login prompt
  } else if (type === 'gmail' || type === 'graph') {
    // Fixed vendor hostnames, so there is no SSRF surface to check here — but a
    // missing token is only discoverable at the login prompt otherwise.
    if (!config.refreshToken) throw Object.assign(new Error(`a ${type} refreshToken is required`), { status: 400 });
    if (!config.clientId) throw Object.assign(new Error(`a ${type} clientId is required`), { status: 400 });
  } else {
    // The relay URL is caller-supplied and the server fetches it, so it is an
    // SSRF primitive: rejected here so the tenant sees why, and again at fetch
    // time because a public name can be re-pointed at an internal address.
    if (!config.url) throw Object.assign(new Error(`a ${type} relay url is required`), { status: 400 });
    await assertSafeTarget(config.url, { protocols: ['http:', 'https:'], label: 'mfa relay url' });
  }
  const key = keyFor(personaId, domain);
  configs.set(key, sealText(scopeFor(key), config));
  persist();
  return describe(personaId, domain);
}

export function clear(personaId, domain = null) {
  const removed = configs.delete(keyFor(personaId, domain));
  if (removed) persist();
  return removed;
}

/** Drop every factor for a persona, site-specific ones included. */
export function clearAll(personaId) {
  let removed = 0;
  for (const key of [...configs.keys()]) {
    if (key === personaId || key.startsWith(`${personaId}|`)) { configs.delete(key); removed++; }
  }
  if (removed) persist();
  return removed;
}

/** Whether a factor is configured — never what it is. */
export function describe(personaId, domain = null) {
  const found = resolve(personaId, domain);
  if (!found) return { configured: false };
  return { configured: true, type: found.config.type, ...(found.domain ? { domain: found.domain } : {}) };
}

/** Every site-specific factor this persona holds, types only. */
export function list(personaId) {
  const out = [];
  for (const key of configs.keys()) {
    if (!key.startsWith(`${personaId}|`)) continue;
    const domain = key.slice(personaId.length + 1);
    out.push({ domain, type: openText(scopeFor(key), configs.get(key)).type });
  }
  return out.sort((a, b) => a.domain.localeCompare(b.domain));
}

/** The site's own factor, else the persona-wide one. */
function resolve(personaId, domain) {
  for (const candidate of [domain ? keyFor(personaId, domain) : null, String(personaId)].filter(Boolean)) {
    const sealed = configs.get(candidate);
    if (sealed) return { config: openText(scopeFor(candidate), sealed), domain: candidate === String(personaId) ? null : domain };
  }
  return null;
}

function load(personaId, domain) { return resolve(personaId, domain)?.config || null; }

// ── Detection ──

/** Is the page asking for a second factor, and where does the code go? */
export const DETECT_JS = `(() => {
  const fields = [...document.querySelectorAll('input')].filter((el) => {
    if (el.type === 'hidden' || el.disabled || el.readOnly || !el.getClientRects().length || getComputedStyle(el).visibility === 'hidden') return false;
    const hay = [el.name, el.id, el.autocomplete, el.placeholder, el.getAttribute('aria-label')]
      .filter(Boolean).join(' ').toLowerCase();
    if (/\\b(otp|one[- ]?time|2fa|two[- ]?factor|mfa|verification|auth(entication)?[- ]?code|security[- ]?code|passcode)\\b/.test(hay)) return true;
    // Compact names glue the token to a word: totp, otpCode, mfaCode, totpmfa. It must still
    // open or close a word, so "footprint" and "hotpink" stay out.
    if (/(?:^|[^a-z])(?:t?otp|mfa|2fa)|(?:t?otp|mfa|2fa)(?:$|[^a-z])/.test(hay)) return true;
    if (el.autocomplete === 'one-time-code') return true;
    // A short numeric field on a page that talks about codes.
    const maxLen = Number(el.maxLength);
    return maxLen > 0 && maxLen <= 8 && /^(text|tel|number)$/.test(el.type)
      && /\\b(code|verify|verification)\\b/i.test(document.body.innerText || '');
  });
  document.querySelectorAll('[data-oya-mfa-target]').forEach((el) => el.removeAttribute('data-oya-mfa-target'));
  if (!fields.length) return { present: /approve (the |this )?(sign.in|request)|check your authenticator|insert your security key/i.test(document.body.innerText || ''), handoff: true };
  const el = fields[0];
  fields.forEach((field) => field.setAttribute('data-oya-mfa-target', '1'));
  return {
    present: true,
    segmented: fields.length > 1 && fields.every((f) => Number(f.maxLength) === 1),
    fieldCount: fields.length,
  };
})()`;

/** Type the code in, including the segmented one-box-per-digit style. */
export const fillCodeJS = (code, segmented) => `(() => {
  const code = ${JSON.stringify(String(code))};
  const fire = (el, v) => {
    el.focus();
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };
  if (${segmented ? 'true' : 'false'}) {
    const boxes = [...document.querySelectorAll('[data-oya-mfa-target]')].filter((f) => Number(f.maxLength) === 1 && !f.disabled);
    if (boxes.length < code.length) return { filled: false, reason: 'not enough inputs' };
    code.split('').forEach((ch, i) => fire(boxes[i], ch));
    return { filled: true, segmented: true };
  }
  const el = document.querySelector('[data-oya-mfa-target]');
  if (!el) return { filled: false, reason: 'field not found' };
  fire(el, code);
  return { filled: true, segmented: false };
})()`;

// ── Code retrieval ──

/**
 * Read a one-time code from a mailbox or SMS endpoint.
 *
 * Polled with a bounded window: the code is sent in response to the login
 * attempt, so it does not exist yet when the prompt appears.
 *
 * `since` is what stops the previous run's code being handed back. These codes
 * expire in minutes and the portals offer a Resend button, so an old one is not
 * merely stale — it fails in a way that reads like a broken detector. A relay
 * that cannot say when its message arrived is trusted only for messages it
 * returns after this call started polling.
 */
/**
 * Pull the code out of a message.
 *
 * A regex is the wrong primary tool here: portals rewrite these templates
 * constantly, codes are not always digits, and a verification email is full of
 * other numbers — a case reference, a phone number, "valid for 5 minutes". The
 * tenant's own LLM reads it instead, with the regex kept as the fallback for
 * self-hosters with no LLM key configured and for when the call fails.
 *
 * Only the one already-matched message is sent, truncated — never a mailbox.
 * It goes to the same provider the tenant's agent runs on, which already sees
 * page content.
 */
export async function extractCode(text, { llm, pattern } = {}) {
  const body = String(text || '').slice(0, 4000);
  const byPattern = () => {
    const match = body.match(pattern || /\b(\d{4,8})\b/);
    return match ? (match[1] || match[0]) : null;
  };
  if (!llm?.openaiKey) return byPattern();

  try {
    const completion = await chatCompletion({
      baseUrl: llm.baseUrl,
      apiKey: llm.openaiKey,
      model: llm.model,
      messages: [
        { role: 'system', content: 'You extract one-time verification codes from messages. '
          + 'Reply with the code alone and nothing else — no label, no quotes, no explanation. '
          + 'The code is what the reader is meant to type into a website to finish signing in. '
          + 'It is NOT a case or reference number, an account number, a phone number, an amount, a date, or a duration such as "valid for 5 minutes". '
          + 'Codes are usually 4-8 characters and may contain letters. '
          + 'If the message has no such code, reply exactly: NONE' },
        { role: 'user', content: body },
      ],
    });
    const answer = String(completion.choices?.[0]?.message?.content || '').trim();
    // Trust it only when the answer is shaped like a code. A model that
    // explains itself, or invents one, must not put prose in a login form.
    if (/^[A-Za-z0-9-]{4,10}$/.test(answer) && answer.toUpperCase() !== 'NONE') return answer;
    if (answer.toUpperCase() === 'NONE') return null;
  } catch (e) {
    console.error('[mfa] code extraction fell back to the pattern:', e.message);
  }
  return byPattern();
}

async function fetchRelayCode(config, since = 0, llm = null) {
  const deadline = Date.now() + (Number(config.timeoutMs) || 90_000);
  const pattern = config.pattern ? new RegExp(config.pattern) : null;
  const readInbox = config.type === 'gmail' ? inbox.gmail : config.type === 'graph' ? inbox.graph : null;
  const read = (text) => extractCode(text, { llm, pattern });

  while (Date.now() < deadline) {
    try {
      if (readInbox) {
        const message = await readInbox(config, since);
        const code = message && await read(message.text);
        if (code) return code;
      } else {
        // Re-checked every poll: the name was safe when it was stored, which
        // says nothing about where it resolves now.
        await assertSafeTarget(config.url, { protocols: ['http:', 'https:'], label: 'mfa relay url' });
        const res = await fetch(config.url, {
          headers: config.headers || {},
          redirect: 'error',        // a 30x into an internal address would bypass the check above
          signal: AbortSignal.timeout(15_000),
        });
        if (res.ok) {
          const body = await res.text();
          // An endpoint that timestamps its message is taken at its word; one
          // that does not is only trusted from the second poll onward, by which
          // point anything it returns arrived after this login.
          const at = Number(res.headers.get('x-oya-received-at')) || 0;
          if (!at || at >= since) {
            const code = await read(body);
            if (code) return code;
          }
        }
      }
    } catch (e) {
      // A revoked refresh token never recovers by polling, and burning the
      // whole window on it hides the one message that would fix it.
      if (e.status === 502) throw e;
    }
    await new Promise((r) => setTimeout(r, 5000));
  }
  throw Object.assign(new Error('No one-time code arrived within the window'), { status: 504 });
}

/**
 * The code in what a person replied with, or null.
 *
 * They answer a parked run in a chat box, so the reply is "445566", or
 * "the code is K7R4QP", or "done" meaning they finished it in the live view
 * themselves. A code always carries a digit, which is what keeps "done",
 * "ok" and "finished" out of a login form.
 */
export function codeInReply(reply) {
  const text = String(reply || '').trim();
  const looksLikeCode = (token) => /^[A-Za-z0-9][A-Za-z0-9-]{3,9}$/.test(token) && /\d/.test(token);
  if (looksLikeCode(text)) return text;
  for (const token of text.split(/[^A-Za-z0-9-]+/)) if (looksLikeCode(token)) return token;
  return null;
}

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
  if (segmented === null) {
    const found = await evaluate(DETECT_JS);
    if (!found?.present || found.handoff) return { present: false, filled: false, submitted: false, completed: false };
    segmented = !!found.segmented;
  }
  const filled = await evaluate(fillCodeJS(code, segmented));
  if (!filled?.filled) return { present: true, filled: false, submitted: false, completed: false, reason: filled?.reason };

  const submitted = !!await evaluate(`(() => {
    const el = document.querySelector('[data-oya-mfa-target]');
    if (!el) return false;
    const form = el.form;
    const button = [...(form || document).querySelectorAll('button, input[type="submit"]')].find((b) =>
      !b.disabled && b.getClientRects().length && /^(verify|confirm|continue|submit|sign in|log in)( code)?$/i.test((b.innerText || b.value || '').trim()));
    if (button) { button.click(); return true; }
    if (form) { form.requestSubmit(); return true; }
    return false;
  })()`);

  // Filling an input is not proof the site accepted a factor. A disappearing
  // challenge after submission is the observable success signal.
  let completed = false;
  const deadline = Date.now() + (submitted ? 10_000 : 0);
  do {
    try { completed = !(await evaluate(DETECT_JS))?.present; } catch { /* navigation */ }
    if (completed || Date.now() >= deadline) break;
    await new Promise((r) => setTimeout(r, 250));
  } while (true);
  return { present: true, filled: true, submitted, completed };
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
  const found = await evaluate(DETECT_JS);
  if (!found?.present) return { present: false, completed: false, method: 'none' };

  const config = load(personaId, domain);
  if (!config || found.handoff) {
    metrics.mfaCompleted.inc({ method: 'handoff', outcome: 'needed' });
    return {
      present: true, completed: false, method: 'handoff', liveViewUrl,
      error: `No MFA factor is configured for ${domain ? `${domain} or ` : ''}this persona. Open the live view to complete it by hand.`,
    };
  }

  let code;
  try {
    code = config.type === 'totp' ? totp(config.secret) : await fetchRelayCode(config, since, llm);
  } catch (err) {
    metrics.mfaCompleted.inc({ method: config.type, outcome: 'error' });
    return { present: true, completed: false, method: config.type, liveViewUrl, error: err.message };
  }

  const { filled, submitted, completed } = await submitCode(evaluate, code, !!found.segmented);
  metrics.mfaCompleted.inc({ method: config.type, outcome: completed ? 'ok' : 'needs_attention' });
  return {
    present: true, completed, filled, submitted, method: config.type, segmented: !!found.segmented,
    ...(completed ? {} : { liveViewUrl, error: filled ? 'The code was entered, but the site has not confirmed it. Open the live view to finish.' : 'Could not fill the code field' }),
  };
}

/** Test hook. */
export function reset() { configs.clear(); }
