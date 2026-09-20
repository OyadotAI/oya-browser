/**
 * The page side of MFA: scripts that run in the browser to find a code prompt,
 * type the code in and submit it.
 */

/** Is the page asking for a second factor, and where does the code go? */
export const DETECT_JS = `(() => {
  if (document.readyState !== 'complete') return { present: false, loading: true };
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

/** Code typer, up to the code. */
const FILL_CODE_HEAD = `(() => {
  const code = `;
/** Code typer, between the code and the segmented flag. */
const FILL_CODE_MIDDLE = `;
  const fire = (el, v) => {
    el.focus();
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };
  if (`;
/** Code typer, after the segmented flag. */
const FILL_CODE_TAIL = `) {
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

/** Type the code in, including the segmented one-box-per-digit style. */
export const fillCodeJS = (code, segmented) =>
  `${FILL_CODE_HEAD}${JSON.stringify(String(code))}${FILL_CODE_MIDDLE}${segmented ? 'true' : 'false'}${FILL_CODE_TAIL}`;

/** Press the challenge's verify button, or submit its form. True when something was pressed. */
export const SUBMIT_CODE_JS = `(() => {
    const el = document.querySelector('[data-oya-mfa-target]');
    if (!el) return false;
    const form = el.form;
    const button = [...(form || document).querySelectorAll('button, input[type="submit"]')].find((b) =>
      !b.disabled && b.getClientRects().length && /^(verify|confirm|continue|submit|sign in|log in)( code)?$/i.test((b.innerText || b.value || '').trim()));
    if (button) { button.click(); return true; }
    if (form) { form.requestSubmit(); return true; }
    return false;
  })()`;
