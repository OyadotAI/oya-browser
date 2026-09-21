#!/usr/bin/env node
/**
 * Automatic portal sign-in: credentials, per-site factors, mailbox codes.
 *
 * The cookie jar is still how a persona stays signed in; this covers the
 * fallback for portals that drop the session between runs. The parts that must
 * hold: a password is never readable back, a refused password is never typed a
 * second time (that is how a real clinical account gets locked), a factor is
 * chosen per site, and a code from a previous run is never reused.
 *
 * The live half is skipped when no Chrome binary is present.
 */

import { createServer } from 'http';
import { spawn } from 'child_process';
import { once } from 'events';
import { mkdtempSync, existsSync, readFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

process.env.OYA_DATA_DIR = mkdtempSync(join(tmpdir(), 'oya-login-data-'));
// The relay fixture is on loopback, which net-guard refuses by default and
// should, this is the documented single-operator opt-out, scoped to this test.
process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';

const credentials = await import('../../src/modules/personas/credentials.ts');
const mfa = await import('../../src/modules/challenges/mfa.ts');
const login = await import('../../src/modules/challenges/login.ts');
const inbox = await import('../../src/modules/challenges/inbox.ts');
const { CDPDriver } = await import('../../src/drivers/cdp.ts');
const { removeScratch } = await import('../support/scratch.js');

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
const PERSONA = 'p-testlogin0001';
const OTHER = 'p-testlogin0002';

console.log('\n1️⃣  Credentials are sealed, scoped, and write-only...');
credentials.set(PERSONA, 'https://www.portal.example.net/auth/Login.aspx', {
  username: 'alice',
  password: 'hunter2-secret',
});
credentials.set(PERSONA, 'example.com', { username: 'bob', password: 'correct-horse' });

const described = credentials.describe(PERSONA, 'portal.example.net');
assert(described.configured && described.username === 'alice', 'describe() names the account bound to the site');
assert(!JSON.stringify(described).includes('hunter2'), 'describe() never carries the password');
assert(!JSON.stringify(credentials.list(PERSONA)).includes('hunter2'), 'list() never carries the password');
assert(
  credentials.domainOf('https://www.portal.example.net/x') === 'portal.example.net',
  'a URL files under its host without www',
);

const onDisk = readFileSync(join(process.env.OYA_DATA_DIR, 'credentials.json'), 'utf8');
assert(!onDisk.includes('hunter2') && !onDisk.includes('correct-horse'), 'no plaintext password reaches the disk');

assert(credentials.lookup(PERSONA, 'portal.example.net').password === 'hunter2-secret', 'the server can still open it');
assert(
  credentials.lookup(PERSONA, 'login.example.com')?.username === 'bob',
  'a subdomain falls back to the registrable parent',
);
assert(credentials.lookup(OTHER, 'portal.example.net') === null, 'another persona sees nothing');

credentials.restore();
assert(credentials.lookup(PERSONA, 'portal.example.net')?.username === 'alice', 'survives a reload from disk');

console.log('\n2️⃣  A factor per site, with the persona-wide one as fallback...');
const SEED = 'JBSWY3DPEHPK3PXP';
await mfa.set(PERSONA, { type: 'totp', secret: SEED }); // persona-wide
await mfa.set(PERSONA, { type: 'totp', secret: 'KRSXG5CTMVRXEZLU' }, 'example.com');
assert(mfa.describe(PERSONA, 'example.com').domain === 'example.com', 'the site factor wins for its own site');
assert(
  mfa.describe(PERSONA, 'portal.example.net').configured && !mfa.describe(PERSONA, 'portal.example.net').domain,
  'a site with no factor of its own falls back to the persona-wide one',
);
assert(mfa.describe(PERSONA).configured, 'the persona-wide factor is still addressable on its own');
assert(
  mfa.list(PERSONA).length === 1 && mfa.list(PERSONA)[0].domain === 'example.com',
  'list() shows only site factors',
);
assert(
  !readFileSync(join(process.env.OYA_DATA_DIR, 'mfa.json'), 'utf8').includes('JBSW'),
  'no plaintext seed reaches the disk',
);

console.log('\n3️⃣  Mailbox parsing, no network...');
const b64url = (s) => Buffer.from(s, 'utf8').toString('base64').replace(/\+/g, '-').replace(/\//g, '_');
assert(
  inbox
    .gmailBody({
      mimeType: 'multipart/alternative',
      parts: [
        { mimeType: 'text/html', body: { data: b64url('<p>Your code is <b>445566</b></p>') } },
        { mimeType: 'text/plain', body: { data: b64url('Your code is 112233') } },
      ],
    })
    .includes('112233'),
  'gmail prefers the plain-text part',
);
assert(
  inbox.gmailBody({ mimeType: 'text/html', body: { data: b64url('<style>x{}</style><p>code 778899</p>') } }) ===
    'code 778899',
  'an html-only body is reduced to its text',
);
assert(inbox.htmlToText('<p>a&nbsp;&amp;&nbsp;b</p>') === 'a & b', 'entities decode');

console.log('\n4️⃣  The code is read by the LLM, not a regex...');
{
  const EMAIL = [
    'Benefits management portal: verification',
    'Reference number 20240917-88421 for case 5551234.',
    'Your one-time access code is K7R4QP.',
    'It expires in 5 minutes. Do not share it. Questions? Call 1-800-555-0199.',
  ].join('\n');

  // No LLM configured: the pattern is all there is, and it picks a number that
  // is not the code, which is exactly why the LLM is the primary path.
  const byPattern = await mfa.extractCode(EMAIL, {});
  assert(byPattern !== 'K7R4QP', `the pattern cannot find a non-numeric code (got ${byPattern})`);

  // A stub LLM speaking the OpenAI shape, so no network and no key needed.
  const llmCalls = [];
  const stub = createServer((req, res) => {
    let body = '';
    req.on('data', (d) => {
      body += d;
    });
    req.on('end', () => {
      llmCalls.push(JSON.parse(body));
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ choices: [{ message: { content: 'K7R4QP' } }] }));
    });
  });
  await new Promise((r) => stub.listen(0, '127.0.0.1', r));
  const llm = { openaiKey: 'sk-test', baseUrl: `http://127.0.0.1:${stub.address().port}`, model: 'test-model' };

  assert(
    (await mfa.extractCode(EMAIL, { llm })) === 'K7R4QP',
    'the LLM picks the code out of a message full of other numbers',
  );
  assert(llmCalls[0].messages[1].content.includes('K7R4QP'), 'the message text is what it was asked about');
  assert(llmCalls[0].messages[1].content.length <= 4000, 'and it is truncated, never a whole mailbox');

  // A model that answers with prose, or invents one, must not reach a login form.
  const chatty = createServer((req, res) => {
    req.resume();
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(
      JSON.stringify({
        choices: [{ message: { content: 'The verification code is 998877, valid for five minutes.' } }],
      }),
    );
  });
  await new Promise((r) => chatty.listen(0, '127.0.0.1', r));
  const chattyLlm = { ...llm, baseUrl: `http://127.0.0.1:${chatty.address().port}` };
  assert(
    (await mfa.extractCode('code 112233', { llm: chattyLlm })) === '112233',
    'an answer that is not shaped like a code falls back to the pattern',
  );

  const refusing = createServer((req, res) => {
    req.resume();
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ choices: [{ message: { content: 'NONE' } }] }));
  });
  await new Promise((r) => refusing.listen(0, '127.0.0.1', r));
  assert(
    (await mfa.extractCode('Your invoice 445566 is ready.', {
      llm: { ...llm, baseUrl: `http://127.0.0.1:${refusing.address().port}` },
    })) === null,
    'NONE means no code, rather than grabbing the nearest number',
  );

  const broken = createServer((req, res) => {
    req.resume();
    res.writeHead(500);
    res.end('nope');
  });
  await new Promise((r) => broken.listen(0, '127.0.0.1', r));
  assert(
    (await mfa.extractCode('code 334455', {
      llm: { ...llm, baseUrl: `http://127.0.0.1:${broken.address().port}` },
    })) === '334455',
    'an LLM that is down falls back to the pattern rather than failing the login',
  );

  for (const server of [stub, chatty, refusing, broken]) await new Promise((r) => server.close(r));
}

console.log("\n5️⃣  A person's reply is read without typing 'done' into the form...");
for (const [reply, expected] of [
  ['445566', '445566'],
  ['K7R4QP', 'K7R4QP'],
  ['the code is K7R4QP', 'K7R4QP'],
  ['Code: 112233, expires soon', '112233'],
  ['done', null],
  ['ok', null],
  ['finished in the live view', null],
  ['', null],
]) {
  const got = mfa.codeInReply(reply);
  assert(got === expected, `${JSON.stringify(reply)} -> ${JSON.stringify(got)} (wanted ${JSON.stringify(expected)})`);
}

console.log('\n6️⃣  A code from the previous run is never reused...');
{
  // A relay that timestamps its message: the stale one is ignored, the fresh one taken.
  let served = 0;
  const relay = createServer((req, res) => {
    served++;
    const stale = served === 1;
    res.writeHead(200, {
      'Content-Type': 'text/plain',
      'x-oya-received-at': String(Date.now() - (stale ? 600_000 : 0)),
    });
    res.end(stale ? 'Your code is 111111' : 'Your code is 222222');
  });
  await new Promise((r) => relay.listen(0, '127.0.0.1', r));
  const url = `http://127.0.0.1:${relay.address().port}/`;
  await mfa.set(PERSONA, { type: 'email', url, timeoutMs: 20_000 }, 'stale.example');

  let typed = null;
  const fake = (expr) => {
    if (expr.includes('data-oya-mfa-target')) {
      const m = expr.match(/const code = "(\d+)"/);
      if (m) {
        typed = m[1];
        return { filled: true };
      }
    }
    // The detector, matched on its body rather than its first line: it now
    // opens with a readyState guard, and keying a fake to that is how a test
    // breaks on a change that did not touch it.
    if (expr.includes('const fields = [...document.querySelectorAll')) {
      return { present: !typed, segmented: false, fieldCount: 1 };
    }
    return false;
  };
  const r = await mfa.complete(fake, PERSONA, { domain: 'stale.example', since: Date.now() - 60_000 });
  assert(typed === '222222', `the stale code was skipped and the fresh one used (typed ${typed})`);
  assert(r.completed, 'and the challenge cleared');
  await new Promise((r2) => relay.close(r2));
}

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].find((p) => existsSync(p));

if (!CHROME) {
  console.log('\n⏭  No Chrome binary found, skipping the live sign-in test');
  removeScratch(process.env.OYA_DATA_DIR);
  console.log(`\n  ${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

// ── Live fixtures ────────────────────────────────────────────────────────────

let posts = 0;
const form = (msg = '') => `<!doctype html><title>portal</title>
${msg ? `<p class="err">${msg}</p>` : ''}
<form method="POST" action="/login">
  <input name="user" type="text" placeholder="User ID">
  <input name="pass" type="password" placeholder="Password">
  <button type="submit">Log In</button>
</form>`;

const site = createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (req.method === 'POST') {
    posts++;
    let body = '';
    req.on('data', (d) => {
      body += d;
    });
    req.on('end', () => {
      const params = new URLSearchParams(body);
      const ok = params.get('user') === 'alice' && params.get('pass') === 'hunter2-secret';
      if (url.pathname === '/login' && ok) {
        // Two-stage, the shape these portals actually use: password, then an
        // explicit request for the code, then the code itself.
        res.writeHead(200, { 'Content-Type': 'text/html' });
        // What these portals actually show between the password and the code:
        // a masked recipient and a validity notice, with an explicit request
        // button, as these portals do.
        return res.end(
          '<!doctype html><title>portal</title><p>We will send a code to a\u2022\u2022\u2022\u2022e@example.com. It is valid for five minutes.</p><button id="send">Send Email</button>',
        );
      }
      res.writeHead(200, { 'Content-Type': 'text/html' });
      return res.end(form('The user ID or password is incorrect.'));
    });
    return;
  }
  res.writeHead(200, { 'Content-Type': 'text/html' });
  if (url.pathname === '/locked') return res.end(form('This account is locked. Contact your administrator.'));
  if (url.pathname === '/code')
    return res.end(
      '<!doctype html><title>portal</title><input autocomplete="one-time-code" maxlength="6"><button>Verify Code</button>',
    );
  // An ordinary page that happens to have a "Send Email" button on it. Nothing
  // here is a login, and clicking it would be the automation acting on its own.
  if (url.pathname === '/contact')
    return res.end(
      '<!doctype html><title>portal</title><h1>Contact your account manager</h1><p>We usually reply within two business days.</p><button>Send Email</button>',
    );
  res.end(form());
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'oya-login-'));
const chrome = spawn(
  CHROME,
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
  let buf = '';
  const timer = setTimeout(() => reject(new Error('Chrome did not report a DevTools endpoint')), 20000);
  chrome.stderr.on('data', (d) => {
    buf += d.toString();
    const m = buf.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
    if (m) {
      clearTimeout(timer);
      resolve(m[0]);
    }
  });
  chrome.on('exit', () => {
    clearTimeout(timer);
    reject(new Error('Chrome exited early'));
  });
});

let driver;
try {
  driver = await new CDPDriver({ wsUrl, provider: 'chrome' }).connect();
  const evaluate = (expr) => driver.evaluateMain(expr);
  credentials.set(PERSONA, '127.0.0.1', { username: 'alice', password: 'hunter2-secret' });

  console.log('\n7️⃣  A real sign-in form is filled and submitted...');
  await driver.send('navigate', { url: siteUrl });
  const r1 = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b1' });
  assert(r1.completed, `the form was accepted and went away (${r1.error || 'ok'})`);
  assert(r1.username === 'alice', 'the result names the account used, never the password');
  assert(!JSON.stringify(r1).includes('hunter2'), 'the password is not in the result');
  assert(posts === 1, `exactly one submission (saw ${posts})`);

  console.log('\n8️⃣  The code-request step portals put in the middle...');
  const r2 = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b1' });
  assert(r2.method === 'request_code' && r2.completed, 'the Send Email button is found and clicked');
  assert(typeof r2.requestedAt === 'number', 'and it timestamps the request, so a stale code cannot answer it');

  console.log('\n9️⃣  A refused password is never typed twice...');
  posts = 0;
  credentials.set(PERSONA, '127.0.0.1', { username: 'alice', password: 'wrong-password' });
  await driver.send('navigate', { url: siteUrl });
  const bad = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b2' });
  assert(!bad.completed && bad.rejected, 'the rejection is reported, not retried into');
  assert(posts === 1, `the account took exactly one bad attempt (saw ${posts})`);
  const again = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b2' });
  assert(again.rejected && !again.completed, 'a second pass still refuses');
  assert(posts === 1, `and still no second submission (saw ${posts})`);

  console.log('\n🔟  A locked account stops everything...');
  await driver.send('navigate', { url: `${siteUrl}locked` });
  const locked = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b3' });
  assert(locked.locked && locked.method === 'handoff', 'a lockout is a handoff, not another attempt');
  assert(posts === 1, `nothing was submitted to a locked account (saw ${posts})`);

  console.log('\n1️⃣1️⃣  A "Send Email" button off a login page is left alone...');
  await driver.send('navigate', { url: `${siteUrl}contact` });
  const contact = await login.complete(evaluate, PERSONA, { domain: '127.0.0.1', browserId: 'b5' });
  assert(!contact.present, 'a contact form is not mistaken for a sign-in step');
  assert(
    (await driver.evaluateMain('document.querySelector("h1").textContent')) === 'Contact your account manager',
    'and its button was never clicked',
  );

  console.log('\n1️⃣2️⃣  No credentials is a handoff, not a failure...');
  await driver.send('navigate', { url: siteUrl });
  const none = await login.complete(evaluate, OTHER, { domain: '127.0.0.1', browserId: 'b4', liveViewUrl: '/live/x' });
  assert(none.method === 'handoff' && none.liveViewUrl === '/live/x', 'it hands over with somewhere to go');

  console.log('\n1️⃣3️⃣  A person who replies with the code has answered it...');
  await driver.send('navigate', { url: `${siteUrl}code` });
  // No segmented hint and no prior detect: the same call api.js makes after a
  // person replies to a parked run, half an hour and a navigation later.
  const typed = await mfa.submitCode(evaluate, '445566');
  assert(typed.filled, 'the code field takes a human-supplied code with no prior detection pass');
  assert((await driver.evaluateMain('document.querySelector("input").value')) === '445566', 'and it lands in the page');
} catch (e) {
  console.log(`  ❌ threw: ${e.message}`);
  failed++;
} finally {
  driver?.close();
  const exited = once(chrome, 'exit');
  chrome.kill('SIGTERM');
  await exited;
  await new Promise((r) => site.close(r));
  removeScratch(profile);
  removeScratch(process.env.OYA_DATA_DIR);
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
