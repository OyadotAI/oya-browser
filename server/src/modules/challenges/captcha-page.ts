/**
 * The page side of CAPTCHA handling: scripts that run in the browser to find a
 * challenge and to hand a solved token back to it.
 */
import { MIN_CHALLENGE_HEIGHT_PX, MIN_CHALLENGE_WIDTH_PX } from './constants.ts';

/**
 * Find a challenge and read what a solver needs from it. Runs in the page.
 * Deliberately read-only — it identifies, it does not interact.
 *
 * `invisible` is the difference between a challenge and a widget. Sites keep a
 * reCAPTCHA on pages that are not asking anything — behind a login form, as the
 * badge in the corner, as the frame that scores the visit — and a run that parks a
 * person on every one of those never finishes. Stack Overflow's question list
 * carries one, and it stopped four replays of a page with nothing to solve.
 */
export const DETECT_JS = `(() => {
  if (document.readyState !== 'complete') return { present: false, loading: true };
  const out = { present: false, type: null, sitekey: null, url: location.href, invisible: false };

  const frameKey = (src, param) => {
    const m = String(src).match(new RegExp('[?&]' + param + '=([^&]+)'));
    return m ? decodeURIComponent(m[1]) : null;
  };

  // Drawn large enough for a person to be answering it, and not hidden.
  const shown = (el) => {
    const box = el.getBoundingClientRect();
    if (box.width < ${MIN_CHALLENGE_WIDTH_PX} || box.height < ${MIN_CHALLENGE_HEIGHT_PX}) return false;
    const style = getComputedStyle(el);
    return style.visibility !== 'hidden' && style.display !== 'none' && Number(style.opacity || '1') > 0;
  };

  // Turnstile
  const ts = document.querySelector('[data-sitekey].cf-turnstile, .cf-turnstile[data-sitekey]')
    || [...document.querySelectorAll('iframe')].find((f) => /challenges\\.cloudflare\\.com/.test(f.src || ''));
  if (ts) {
    out.present = true; out.type = 'turnstile';
    out.invisible = !shown(ts);
    out.sitekey = ts.dataset?.sitekey || frameKey(ts.src, 'k') || null;
    return out;
  }

  // hCaptcha
  const hc = document.querySelector('[data-sitekey].h-captcha, .h-captcha[data-sitekey]')
    || [...document.querySelectorAll('iframe')].find((f) => /hcaptcha\\.com/.test(f.src || ''));
  if (hc) {
    out.present = true; out.type = 'hcaptcha';
    out.invisible = !shown(hc);
    out.sitekey = hc.dataset?.sitekey || frameKey(hc.src, 'sitekey') || null;
    return out;
  }

  // reCAPTCHA — v2 renders a checkbox frame, v3 runs invisibly
  const rc = document.querySelector('[data-sitekey].g-recaptcha, .g-recaptcha[data-sitekey]')
    || [...document.querySelectorAll('iframe')].find((f) => /google\\.com\\/recaptcha/.test(f.src || ''));
  if (rc) {
    out.present = true;
    out.sitekey = rc.dataset?.sitekey || frameKey(rc.src, 'k') || null;
    // What kind it is comes from how the page declared it; whether anyone is being
    // asked to do it also depends on whether it was drawn. A v2 widget nobody can
    // see is still a v2 widget, and solving it as v3 would buy the wrong answer.
    const declaredInvisible = /size=invisible/.test(rc.src || '') || rc.dataset?.size === 'invisible';
    out.type = declaredInvisible ? 'recaptcha_v3' : 'recaptcha_v2';
    out.invisible = declaredInvisible || !shown(rc);
    return out;
  }

  return out;
})()`;

/** Token placer, up to the token. */
const APPLY_TOKEN_HEAD = `(() => {
  const token = `;
/** Token placer, between the token and the challenge type. */
const APPLY_TOKEN_MIDDLE = `;
  const setField = (name) => {
    let ok = false;
    for (const el of document.querySelectorAll('[name="' + name + '"], #' + name)) {
      el.value = token;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      ok = true;
    }
    return ok;
  };

  const type = `;
/** Token placer, after the challenge type. */
const APPLY_TOKEN_TAIL = `;
  let placed = false;
  if (type === 'turnstile') placed = setField('cf-turnstile-response');
  if (type === 'hcaptcha') placed = setField('h-captcha-response') || setField('g-recaptcha-response');
  if (type.startsWith('recaptcha')) placed = setField('g-recaptcha-response');

  // Many integrations only act on the library's own callback, not the field.
  try {
    if (window.___grecaptcha_cfg?.clients) {
      for (const client of Object.values(window.___grecaptcha_cfg.clients)) {
        for (const v of Object.values(client || {})) {
          if (v && typeof v === 'object' && typeof v.callback === 'function') { v.callback(token); placed = true; }
        }
      }
    }
  } catch {}

  return { placed };
})()`;

/** Put a solved token where the page expects it and let the page proceed. */
export const applyTokenJS = (type, token) =>
  `${APPLY_TOKEN_HEAD}${JSON.stringify(token)}${APPLY_TOKEN_MIDDLE}${JSON.stringify(type)}${APPLY_TOKEN_TAIL}`;
