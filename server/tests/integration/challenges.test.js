#!/usr/bin/env node
/**
 * CAPTCHA and MFA against real pages in the patched Oya engine.
 *
 * Detection and code entry are tested for real; the external solver is not
 * called (that costs money and needs a key), so solving is asserted through
 * the no-solver-configured path and the token application is tested directly.
 */

import { createServer } from 'http';

process.env.OYA_PROFILE_SECRET = 'd'.repeat(64);
delete process.env.OYA_CAPTCHA_API_KEY;

const { openNativeFixture } = await import('../support/native-browser.mjs');
const loginPage = await import('../../src/modules/challenges/login-page.ts');
const captcha = await import('../../src/modules/challenges/captcha.ts');
const mfa = await import('../../src/modules/challenges/mfa.ts');

let passed = 0,
  failed = 0;
const assert = (c, label) => {
  if (c) {
    console.log(`  ✅ ${label}`);
    passed++;
  } else {
    console.log(`  ❌ ${label}`);
    failed++;
  }
};

const PAGES = {
  '/turnstile': `<!doctype html><title>t</title><body>
    <div class="cf-turnstile" data-sitekey="0x4AAAAAAADnPIDROrmt1Wwj"></div>
    <input type="hidden" name="cf-turnstile-response"></body>`,
  '/recaptcha': `<!doctype html><title>r</title><body>
    <div class="g-recaptcha" data-sitekey="6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI"></div>
    <textarea name="g-recaptcha-response"></textarea></body>`,
  '/hcaptcha': `<!doctype html><title>h</title><body>
    <div class="h-captcha" data-sitekey="10000000-ffff-ffff-ffff-000000000001"></div>
    <textarea name="h-captcha-response"></textarea></body>`,
  '/clean': `<!doctype html><title>c</title><body><p>nothing here</p></body>`,
  '/otp': `<!doctype html><title>o</title><body><p>Enter your verification code</p>
    <input name="otp" autocomplete="one-time-code" maxlength="6"></body>`,
  '/otp-boxes': `<!doctype html><title>b</title><body><p>Enter the 6-digit code to verify</p>
    ${Array.from({ length: 6 }, () => '<input maxlength="1" type="tel">').join('')}</body>`,
  // authenticationtest.com's TOTP form: the token is glued into the name, no maxlength, no autocomplete.
  '/otp-compact': `<!doctype html><title>t</title><body><input type="email" name="email">
    <input type="password" name="password"><input type="text" name="totpmfa" id="totpmfa" placeholder="123456"></body>`,
  '/otp-lookalike': `<!doctype html><title>f</title><body><input type="text" name="footprint" placeholder="Shoe size"></body>`,
  // Carelon's account-first page: no "sign in" anywhere, only a field that says it holds a username.
  '/account-first': `<!doctype html><title>Portal</title><body><form><h1>User Confirmation</h1>
    <label for="u">USERNAME</label><input id="u" name="asPrimary$ctl00$txtLoginId" autocomplete="username">
    <input type="submit" value="Next"></form></body>`,
  // The same step named only by its title.
  '/account-first-title': `<!doctype html><title>Sign in</title><body><form><label>Email</label>
    <input type="email" name="email"><button>Next</button></form></body>`,
  // A search box beside a Continue button is not a sign-in step.
  '/account-search': `<!doctype html><title>Accounts</title><body><form>
    <input name="account" placeholder="Search accounts"><button>Continue</button></form></body>`,
};

const site = createServer((req, res) => {
  const body = PAGES[req.url] ?? PAGES['/clean'];
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(body);
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${site.address().port}`;

let conn;
try {
  conn = await openNativeFixture();
  const evaluate = (expression) => conn.evaluateMain(expression);
  const goto = async (path) => {
    await conn.send('navigate', { url: base + path });
    await new Promise((r) => setTimeout(r, 350));
  };

  console.log('\n1️⃣  CAPTCHA detection by type...');
  for (const [path, type, key] of [
    ['/turnstile', 'turnstile', '0x4AAAAAAADnPIDROrmt1Wwj'],
    ['/recaptcha', 'recaptcha_v2', '6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI'],
    ['/hcaptcha', 'hcaptcha', '10000000-ffff-ffff-ffff-000000000001'],
  ]) {
    await goto(path);
    const found = await evaluate(captcha.DETECT_JS);
    assert(found.present && found.type === type, `${type} is detected`);
    assert(found.sitekey === key, `${type} sitekey is read (${found.sitekey?.slice(0, 12)}…)`);
  }

  await goto('/clean');
  assert((await evaluate(captcha.DETECT_JS)).present === false, 'a clean page reports no challenge');

  console.log('\n2️⃣  A missing solver is reported, not swallowed...');
  await goto('/turnstile');
  const unconfigured = await captcha.handle(evaluate);
  assert(unconfigured.present === true, 'the challenge is still reported');
  assert(
    unconfigured.solved === false && unconfigured.method === 'none',
    'solved:false with method none, an agent can act on that',
  );
  assert(/OYA_CAPTCHA_API_KEY/.test(unconfigured.error || ''), 'the error says what to configure');

  const native = await captcha.handle(evaluate, { providerSolves: true });
  assert(native.method === 'provider' && !native.solved, 'a provider that solves natively is not solved over the top');

  console.log('\n3️⃣  A solved token reaches the page...');
  const applied = await evaluate(captcha.applyTokenJS('turnstile', 'test-token-123'));
  assert(applied.placed === true, 'the token is placed in the response field');
  assert(
    (await evaluate(`document.querySelector('[name="cf-turnstile-response"]').value`)) === 'test-token-123',
    'and the field actually holds it',
  );

  console.log('\n4️⃣  MFA detection and code entry...');
  await goto('/otp');
  const otp = await evaluate(mfa.DETECT_JS);
  assert(otp.present === true, 'a one-time-code field is detected');
  assert(otp.segmented === false, 'a single field is not treated as segmented');

  await goto('/otp-boxes');
  const boxes = await evaluate(mfa.DETECT_JS);
  assert(boxes.present === true && boxes.segmented === true, 'six single-character boxes are detected as segmented');

  await goto('/otp-compact');
  assert((await evaluate(mfa.DETECT_JS)).present === true, 'a compact field name like "totpmfa" is detected');
  await goto('/otp-lookalike');
  assert((await evaluate(mfa.DETECT_JS)).present === false, 'a word merely containing "otp" ("footprint") is not');

  await goto('/clean');
  assert((await evaluate(mfa.DETECT_JS)).present === false, 'a clean page reports no MFA prompt');

  console.log('\n5️⃣  TOTP end to end...');
  const persona = 'p-test-persona';
  await mfa.set(persona, { type: 'totp', secret: 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ' });
  assert(mfa.describe(persona).type === 'totp', 'the factor is configured');
  assert(JSON.stringify(mfa.describe(persona)).includes('GEZDG') === false, 'the secret is never returned');

  await goto('/otp');
  const done = await mfa.complete(evaluate, persona);
  assert(
    done.filled === true && done.completed === false && done.method === 'totp',
    'a filled code without site confirmation is not reported as complete',
  );
  const typedCode = await evaluate(`document.querySelector('[name="otp"]').value`);
  assert(/^\d{6}$/.test(typedCode), `a six-digit code was typed (${typedCode})`);
  assert(typedCode === mfa.totp('GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ'), 'and it is the correct code for now');

  await goto('/otp-boxes');
  const segmented = await mfa.complete(evaluate, persona);
  assert(segmented.filled === true, 'segmented inputs are filled too');
  const joined = await evaluate(`[...document.querySelectorAll('input')].map(i=>i.value).join('')`);
  assert(/^\d{6}$/.test(joined), `each box got one digit (${joined})`);

  console.log('\n6️⃣  An account-first sign-in page is found by its text, title or username field...');
  for (const path of ['/account-first', '/account-first-title']) {
    await goto(path);
    const found = await evaluate(loginPage.DETECT_JS);
    assert(found.present && found.stage === 'username', `${path} is an account-first sign-in step`);
  }
  await goto('/account-search');
  assert((await evaluate(loginPage.DETECT_JS)).present === false, 'a search box with Continue is not a sign-in step');

  console.log('\n7️⃣  Nothing to answer it with is a handoff, not a failure...');
  mfa.reset();
  await goto('/otp');
  const handoff = await mfa.complete(evaluate, 'p-no-factor', { liveViewUrl: '/api/live/abc' });
  assert(handoff.method === 'handoff', 'an unconfigured persona gets a handoff');
  assert(handoff.liveViewUrl === '/api/live/abc', 'with a live view a person can finish in');
} catch (e) {
  console.log(`  ❌ threw: ${e.message}`);
  failed++;
} finally {
  await conn?.close();
  await new Promise((r) => site.close(r));
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
