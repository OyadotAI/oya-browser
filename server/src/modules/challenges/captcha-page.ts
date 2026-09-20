/**
 * The page side of CAPTCHA handling: scripts that run in the browser to find a
 * challenge and to hand a solved token back to it.
 */

/**
 * Find a challenge and read what a solver needs from it. Runs in the page.
 * Deliberately read-only — it identifies, it does not interact.
 */
export const DETECT_JS = `(() => {
  if (document.readyState !== 'complete') return { present: false, loading: true };
  const out = { present: false, type: null, sitekey: null, url: location.href, invisible: false };

  const frameKey = (src, param) => {
    const m = String(src).match(new RegExp('[?&]' + param + '=([^&]+)'));
    return m ? decodeURIComponent(m[1]) : null;
  };

  // Turnstile
  const ts = document.querySelector('[data-sitekey].cf-turnstile, .cf-turnstile[data-sitekey]')
    || [...document.querySelectorAll('iframe')].find((f) => /challenges\\.cloudflare\\.com/.test(f.src || ''));
  if (ts) {
    out.present = true; out.type = 'turnstile';
    out.sitekey = ts.dataset?.sitekey || frameKey(ts.src, 'k') || null;
    return out;
  }

  // hCaptcha
  const hc = document.querySelector('[data-sitekey].h-captcha, .h-captcha[data-sitekey]')
    || [...document.querySelectorAll('iframe')].find((f) => /hcaptcha\\.com/.test(f.src || ''));
  if (hc) {
    out.present = true; out.type = 'hcaptcha';
    out.sitekey = hc.dataset?.sitekey || frameKey(hc.src, 'sitekey') || null;
    return out;
  }

  // reCAPTCHA — v2 renders a checkbox frame, v3 runs invisibly
  const rc = document.querySelector('[data-sitekey].g-recaptcha, .g-recaptcha[data-sitekey]')
    || [...document.querySelectorAll('iframe')].find((f) => /google\\.com\\/recaptcha/.test(f.src || ''));
  if (rc) {
    out.present = true;
    out.sitekey = rc.dataset?.sitekey || frameKey(rc.src, 'k') || null;
    out.invisible = /size=invisible/.test(rc.src || '') || rc.dataset?.size === 'invisible';
    out.type = out.invisible ? 'recaptcha_v3' : 'recaptcha_v2';
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
