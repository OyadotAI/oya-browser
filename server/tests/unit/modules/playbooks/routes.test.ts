/**
 * Unit tests for the playbook and run routes: saving from a recording, playing
 * with variables, listing, renaming, promoting and deleting, and submitting,
 * polling and answering background runs — each only on the caller's own browsers.
 */
import { describe, it, before, after, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
// The chat rate limit is its own module's concern; with it on, this file's calls would trip it.
process.env.OYA_LIMIT_CHAT_PER_MIN = '0';
const { router } = await import('../../../../src/modules/playbooks/routes.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { allowKey, callRoute, scriptedBrowser } = await import('../../support/agent.ts');

const KEY = 'playbook-routes-key';
const BROWSER = 'b-pb-routes';
const STEPS = [
  { action: 'navigate', url: 'https://a.test/' },
  { action: 'type', text: 'ada@x.test', el: { name: 'email', tag: 'input', type: 'input' } },
];
let forget: () => void;
let browser;

/** Calls the router as the test key. */
const as = (method: string, url: string, body?: any) => callRoute(router, { method, url, key: KEY, body });
/** Lets queued work settle. */
const settle = () => new Promise((r) => setImmediate(r));

describe('playbook routes', () => {
  before(() => (forget = allowKey(KEY)));
  after(() => {
    forget();
    delete process.env.OYA_LIMIT_CHAT_PER_MIN;
  });
  beforeEach(() => {
    keyConfig.reset();
    browser = scriptedBrowser(BROWSER, KEY, (action) => {
      if (action === 'analyze')
        return { ok: true, data: { elements: [{ id: 1, name: 'email', tag: 'input', visible: true }] } };
      if (action === 'evaluate_raw') return { ok: true, data: { result: { present: false } } };
      return { ok: true, data: {} };
    });
    mock.method(control(), 'emit', async () => {});
    mock.method(console, 'log', () => {});
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
  });

  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(router, { url: '/playbooks' })).status, 401);
  });

  it('answers 404 for a browser the caller does not own', async () => {
    const other = scriptedBrowser('b-theirs', 'someone-else');
    const res = await as('POST', '/browsers/b-theirs/playbooks', { name: 'x', steps: STEPS });
    assert.equal(res.status, 404);
    other.disconnect();
  });

  it('saves a recording as a playbook and describes it', async () => {
    const res = await as('POST', `/browsers/${BROWSER}/playbooks`, { name: 'signup', steps: STEPS });
    assert.deepEqual(res.body.variables, ['email']);
    assert.ok(keyConfig.getPlaybook(KEY, 'signup'));
  });

  it('refuses a save with malformed secrets before touching the steps', async () => {
    const res = await as('POST', `/browsers/${BROWSER}/playbooks`, { name: 'x', steps: STEPS, secrets: 'pw' });
    assert.deepEqual([res.status, res.body.error], [400, 'secrets must be an array of variable names']);
  });

  it('answers a recording with nothing to replay with 400', async () => {
    const res = await as('POST', `/browsers/${BROWSER}/playbooks`, { name: 'x', steps: [] });
    assert.equal(res.status, 400);
  });

  it('plays a saved playbook and answers its result', async () => {
    await as('POST', `/browsers/${BROWSER}/playbooks`, { name: 'signup', steps: STEPS });
    const res = await as('POST', `/browsers/${BROWSER}/playbooks/signup/play`, { variables: { email: 'b@c' } });
    assert.deepEqual(res.body, { steps: 2, total: 2, fellBack: false });
    assert.equal(browser.calls.find((c) => c.action === 'type').params.text, 'b@c');
  });

  it('carries a failed play’s status in the body, since the 200 is already out', async () => {
    await keyConfig.savePlaybook(KEY, 'broken', { steps: [{ action: 'teleport' }], defaults: {} });
    const res = await as('POST', `/browsers/${BROWSER}/playbooks/broken/play`, { autoHeal: false });
    assert.equal(res.status, 200);
    assert.equal(res.body.status, 422);
  });

  it('refuses to play an unknown playbook', async () => {
    const res = await as('POST', `/browsers/${BROWSER}/playbooks/ghost/play`, {});
    assert.equal(res.status, 404);
  });

  it('lists, renames, promotes and deletes playbooks', async () => {
    await as('POST', `/browsers/${BROWSER}/playbooks`, { name: 'a', steps: STEPS });
    assert.deepEqual(
      (await as('GET', '/playbooks')).body.playbooks.map((p) => p.name),
      ['a'],
    );
    assert.equal((await as('PATCH', '/playbooks/a', { name: 'b' })).body.name, 'b');
    await keyConfig.savePlaybook(KEY, 'b:draft', { steps: STEPS, name: 'b:draft' });
    assert.equal((await as('POST', '/playbooks/b/promote')).body.name, 'b');
    assert.deepEqual((await as('DELETE', '/playbooks/b')).body, { ok: true });
    assert.deepEqual((await as('GET', '/playbooks')).body.playbooks, []);
  });

  it('answers a missing playbook with 404 on delete', async () => {
    assert.equal((await as('DELETE', '/playbooks/ghost')).status, 404);
  });

  describe('runs', () => {
    it('submits a playbook run and lets its owner poll it to success', async () => {
      await as('POST', `/browsers/${BROWSER}/playbooks`, { name: 'signup', steps: STEPS });
      const submitted = await as('POST', `/browsers/${BROWSER}/runs`, { playbook: 'signup' });
      assert.equal(submitted.status, 202);
      for (let i = 0; i < 20 && (await as('GET', `/runs/${submitted.body.id}`)).body.status === 'running'; i++)
        await settle();
      assert.equal((await as('GET', `/runs/${submitted.body.id}`)).body.status, 'succeeded');
    });

    it('refuses a run with both a prompt and a playbook', async () => {
      const res = await as('POST', `/browsers/${BROWSER}/runs`, { prompt: 'x', playbook: 'y' });
      assert.equal(res.status, 400);
    });

    it('hides another key’s run and refuses to answer one that is not waiting', async () => {
      assert.equal((await as('GET', '/runs/run_unknown')).status, 404);
      const res = await as('POST', '/runs/run_unknown/respond', { response: 'x' });
      assert.equal(res.status, 409);
    });

    it('answers a run parked on a person', async () => {
      await keyConfig.savePlaybook(KEY, 'broken', {
        name: 'broken',
        prompt: 'p',
        steps: [{ action: 'teleport' }],
        defaults: {},
      });
      mock.method(globalThis, 'fetch', async () => new Response('down', { status: 500 }));
      mock.method(console, 'error', () => {});
      process.env.OPENAI_API_KEY = 'sk-host';
      const { body } = await as('POST', `/browsers/${BROWSER}/runs`, { playbook: 'broken' });
      for (let i = 0; i < 20 && (await as('GET', `/runs/${body.id}`)).body.status !== 'needs_attention'; i++)
        await settle();
      assert.equal((await as('GET', `/runs/${body.id}`)).body.attention.reason, 'heal_failed');
      assert.deepEqual((await as('POST', `/runs/${body.id}/respond`, { response: 'done' })).body, { ok: true });
      delete process.env.OPENAI_API_KEY;
    });
  });
});
