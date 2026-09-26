/**
 * Unit tests for src/api/agent.ts: `Oya.signup()` solves the puzzle and saves
 * the key without replacing one, and `oya.desktop.connect()` reuses a running
 * desktop app or pairs one and waits for it.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Oya } from '../../../dist/index.js';
import { BASE, client, fakeFetch, tickUntil } from '../support/fake-fetch.ts';

/** A signup server: an easy puzzle, and an answer that checks the nonce solves it. */
const signupRoutes = () =>
  fakeFetch({
    'GET /api/auth/agent/challenge': { body: { challenge: 'abc', difficulty: 2 } },
    'POST /api/auth/agent/signup': (call) => {
      const { challenge, nonce } = call.body as { challenge: string; nonce: string };
      const ok = createHash('sha256')
        .update(challenge + nonce)
        .digest('hex')
        .startsWith('00');
      return ok
        ? { body: { api_key: 'new-key', claim_url: 'https://x/claim', cloud_browsers: false } }
        : { status: 400 };
    },
  });

describe('Oya.signup', () => {
  let home: string;
  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), 'oya-signup-'));
    process.env.OYA_CONFIG_HOME = home;
  });
  afterEach(() => delete process.env.OYA_CONFIG_HOME);

  it('solves the puzzle, signs up with the email and saves the key', async () => {
    const fake = signupRoutes();
    const result = await Oya.signup({ email: 'me@x.test', baseUrl: BASE, fetch: fake.fetch });
    assert.deepEqual(result, { apiKey: 'new-key', claimUrl: 'https://x/claim', cloudBrowsers: false, saved: true });
    assert.equal((fake.calls[1].body as { email: string }).email, 'me@x.test');
    assert.deepEqual(JSON.parse(readFileSync(join(home, 'config.json'), 'utf8')), { apiKey: 'new-key', baseUrl: BASE });
  });

  it('never replaces a key already saved', async () => {
    writeFileSync(join(home, 'config.json'), JSON.stringify({ apiKey: 'old' }));
    const result = await Oya.signup({ email: 'me@x.test', baseUrl: BASE, fetch: signupRoutes().fetch });
    assert.equal(result.saved, false);
    assert.equal(JSON.parse(readFileSync(join(home, 'config.json'), 'utf8')).apiKey, 'old');
  });

  it('does not save when asked not to', async () => {
    const result = await Oya.signup({ email: 'e@x.test', baseUrl: BASE, fetch: signupRoutes().fetch, save: false });
    assert.equal(result.saved, false);
  });
});

/** A connected desktop app, as the fleet lists it. */
const desktop = { id: 'd1', provider: 'oya-desktop', health: 'ok' };

describe('oya.desktop.connect', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
  afterEach(() => mock.timers.reset());

  it('uses the desktop app when it is already connected, without pairing', async () => {
    const { oya, calls } = client({
      'GET /api/browsers': { body: [desktop] },
      'GET /api/browsers/d1': { body: desktop },
    });
    const browser = await oya.desktop.connect();
    assert.equal(browser.id, 'd1');
    assert.deepEqual(
      await browser.stop(),
      { id: 'd1', ok: true, reused: true },
      "stop() leaves the person's app connected",
    );
    assert.ok(!calls.some((c) => c.path === '/api/pairing'));
  });

  it('pairs with the persona and waits until the app connects', async () => {
    const work = { ...desktop, persona: 'p-work', personaName: 'work' };
    const { oya, calls } = client({
      'GET /api/browsers': [{ body: [] }, { body: [] }, { body: [work] }],
      'POST /api/pairing': { status: 201, body: { code: 'c1' } },
      'GET /api/browsers/d1': { body: work },
    });
    const connecting = oya.desktop.connect({ open: false, persona: 'work' });
    await tickUntil(connecting, 2000);
    assert.equal((await connecting).id, 'd1');
    assert.deepEqual(calls.find((c) => c.path === '/api/pairing')!.body, { persona: 'work' });
  });

  it('reuses a running app already signed in as the persona, by id or name', async () => {
    const work = { ...desktop, persona: 'p-work', personaName: 'work' };
    for (const persona of ['p-work', 'work']) {
      const { oya, calls } = client({ 'GET /api/browsers': { body: [work] }, 'GET /api/browsers/d1': { body: work } });
      assert.equal((await oya.desktop.connect({ persona })).id, 'd1');
      assert.ok(!calls.some((c) => c.path === '/api/pairing'));
    }
  });

  it('switches a running app signed in as another persona, then waits for it', async () => {
    const home = { ...desktop, persona: 'p-home', personaName: 'home' };
    const work = { ...desktop, persona: 'p-work', personaName: 'work' };
    const { oya, calls } = client({
      'GET /api/browsers': [{ body: [home] }, { body: [home] }, { body: [work] }],
      'POST /api/pairing': { status: 201, body: { code: 'c1' } },
      'GET /api/browsers/d1': { body: work },
    });
    const connecting = oya.desktop.connect({ open: false, persona: 'p-work' });
    await tickUntil(connecting, 2000);
    assert.equal((await connecting).id, 'd1');
    assert.deepEqual(calls.find((c) => c.path === '/api/pairing')!.body, { persona: 'p-work' });
  });

  it('says how to install and pair when the app never connects', async () => {
    const { oya } = client({ 'GET /api/browsers': { body: [] }, 'POST /api/pairing': { body: { code: 'c1' } } });
    const connecting = oya.desktop.connect({ open: false, timeoutMs: 4000 });
    await tickUntil(connecting, 2000);
    await assert.rejects(connecting, (e: Error & { status: number }) => {
      assert.equal(e.status, 504);
      assert.match(e.message, /oyabrowser\.com\/#download.*oya:\/\/connect\?code=c1&server=wss%3A%2F%2Foya\.test%2Fws/);
      return true;
    });
  });
});
