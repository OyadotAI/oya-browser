#!/usr/bin/env node
/**
 * End to end: a persona with a TOTP factor signs itself into a portal that
 * asks for a password and then for an authenticator code.
 *
 * The gap this fills: login-auto.test.js configures a totp factor but never
 * types one into a real page. Here mfa.complete() generates the code on the
 * fly, fills it and submits it, and the portal accepts it.
 *
 * The portal verifies with its own implementation over a +/-1 step window, the
 * way a real one tolerates clock skew. The RFC 6238 vectors in
 * tests/unit/modules/challenges/totp.test.ts are what prove the arithmetic;
 * this proves the wiring.
 */

import { createServer } from 'http';
import { spawn } from 'child_process';
import { once } from 'events';
import { createHmac } from 'crypto';
import { mkdtempSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

process.env.OYA_DATA_DIR = mkdtempSync(join(tmpdir(), 'oya-totp-'));
// The fixture portal is on loopback, which net-guard refuses by default.
process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';

const credentials = await import('../../src/modules/personas/credentials.ts');
const mfa = await import('../../src/modules/challenges/mfa.ts');
const login = await import('../../src/modules/challenges/login.ts');
const { CDPDriver } = await import('../../src/drivers/cdp.ts');
const { removeScratch } = await import('../support/scratch.js');
const { getConnection } = await import('../../src/platform/storage/index.ts');

let passed = 0,
  failed = 0;
/** Reports one rule and tallies it. */
const assert = (c, label) => {
  console.log(`  ${c ? '✅' : '❌'} ${label}`);
  c ? passed++ : failed++;
};

const PERSONA = 'p-testtotp000001';
const SEED = 'JBSWY3DPEHPK3PXP';
const PASSWORD = 'hunter2-secret';

/** The portal's own code for one 30s step: a second implementation, not totp.ts. */
function portalCode(step) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  const bits = [...SEED].map((c) => alphabet.indexOf(c).toString(2).padStart(5, '0')).join('');
  const key = Buffer.from((bits.match(/[01]{8}/g) || []).map((b) => parseInt(b, 2)));
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(step));
  const hmac = createHmac('sha1', key).update(counter).digest();
  const offset = hmac[hmac.length - 1] & 0xf;
  return String((hmac.readUInt32BE(offset) & 0x7fffffff) % 1e6).padStart(6, '0');
}

/** Accepts the code for this step or either neighbour, as a real portal does. */
const portalAccepts = (code) => {
  const now = Math.floor(Date.now() / 30_000);
  return [now - 1, now, now + 1].some((s) => portalCode(s) === code);
};

const loginPage = (error = '') => `<!doctype html><title>portal</title>
${error ? `<p class="err">${error}</p>` : ''}
<form method="POST" action="/login">
  <input name="user" type="text" placeholder="User ID">
  <input name="pass" type="password" placeholder="Password">
  <button type="submit">Log In</button>
</form>`;

const codePage = (error = '') => `<!doctype html><title>portal, verify</title>
${error ? `<p class="err">${error}</p>` : ''}
<p>Enter the 6-digit code from your authenticator app.</p>
<form method="POST" action="/verify">
  <input name="code" autocomplete="one-time-code" maxlength="6" placeholder="Authenticator code">
  <button type="submit">Verify</button>
</form>`;

/** Codes the portal was sent, so a wrong or repeated one is visible. */
const submitted = [];

/** Reads a urlencoded body. */
async function body(req) {
  let raw = '';
  for await (const chunk of req) raw += chunk;
  return new URLSearchParams(raw);
}

const site = createServer(async (req, res) => {
  const { pathname } = new URL(req.url, 'http://x');
  res.writeHead(200, { 'Content-Type': 'text/html' });
  if (req.method !== 'POST') return res.end(loginPage());
  const params = await body(req);
  if (pathname === '/login') {
    const ok = params.get('user') === 'alice' && params.get('pass') === PASSWORD;
    return res.end(ok ? codePage() : loginPage('The user ID or password is incorrect.'));
  }
  const code = params.get('code') || '';
  submitted.push(code);
  if (portalAccepts(code))
    return res.end('<!doctype html><title>Signed in as alice</title><h1>Signed in as alice</h1>');
  return res.end(codePage('That code is not right.'));
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

console.log('\n1️⃣  The factor and the login are stored sealed...');
await credentials.set(PERSONA, '127.0.0.1', { username: 'alice', password: PASSWORD });
await mfa.set(PERSONA, { type: 'totp', secret: SEED }, '127.0.0.1');
assert(mfa.describe(PERSONA, '127.0.0.1').type === 'totp', 'the site factor reports its type');
assert(
  !(await getConnection().select('mfa_factors')).some((r) => r.value.includes('JBSW')),
  'and the seed is nowhere in mfa.json as plaintext',
);

const chromePath = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
].find(existsSync);
if (!chromePath) {
  console.log('\n⏭  No Chrome binary; the live half is skipped.');
  await new Promise((r) => site.close(r));
  removeScratch(process.env.OYA_DATA_DIR);
  process.exit(failed ? 1 : 0);
}

const profile = mkdtempSync(join(tmpdir(), 'oya-totp-chrome-'));
const chrome = spawn(
  chromePath,
  [
    '--headless=new',
    '--remote-debugging-port=0',
    '--no-first-run',
    '--no-default-browser-check',
    `--user-data-dir=${profile}`,
    'about:blank',
  ],
  { stdio: ['ignore', 'ignore', 'pipe'] },
);
const wsUrl = await new Promise((resolve, reject) => {
  let out = '';
  const timer = setTimeout(() => reject(new Error('Chrome did not start')), 20_000);
  chrome.stderr.on('data', (d) => {
    out += d;
    const m = out.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
    if (m) (clearTimeout(timer), resolve(m[0]));
  });
  chrome.once('error', reject);
});

let driver;
try {
  driver = await new CDPDriver({ wsUrl, provider: 'chrome' }).connect();
  const evaluate = (expr) => driver.evaluateMain(expr);
  const title = () => driver.evaluateMain('document.title');

  console.log('\n2️⃣  The password gets the portal to ask for a code...');
  await driver.send('navigate', { url: siteUrl });
  const signedIn = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b1' });
  assert(signedIn.completed, `the sign-in form was accepted (${signedIn.error || 'ok'})`);
  assert((await title()).includes('verify'), 'and the page now asks for the authenticator code');

  console.log('\n3️⃣  The code is generated, typed and submitted with nobody watching...');
  const result = await mfa.complete(evaluate, PERSONA, { domain: '127.0.0.1' });
  assert(result.present, 'the prompt was detected');
  assert(result.method === 'totp', `the totp factor answered it (method=${result.method})`);
  assert(result.filled && result.submitted, 'the code was typed in and the form submitted');
  assert(result.completed, `and the portal accepted it (${result.error || 'ok'})`);
  assert((await title()) === 'Signed in as alice', 'the portal is showing the signed-in page');

  console.log('\n4️⃣  Exactly one code, and no secret leaked into the result...');
  assert(submitted.length === 1, `the portal saw one code, not a retry storm (saw ${submitted.length})`);
  assert(/^\d{6}$/.test(submitted[0]), `it was six digits (${submitted[0]})`);
  assert(!JSON.stringify(result).includes(SEED), 'the seed is not in the result object');
  assert(!JSON.stringify(result).includes(submitted[0]), 'and neither is the code it used');
} catch (e) {
  console.log(`  ❌ threw: ${e.message}`);
  failed++;
} finally {
  driver?.close();
  const exited = once(chrome, 'exit');
  chrome.kill('SIGTERM');
  await exited;
  await new Promise((r) => site.close(r));
  removeScratch(process.env.OYA_DATA_DIR);
  removeScratch(profile);
}

console.log(`\n${failed ? '❌' : '✅'} ${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
