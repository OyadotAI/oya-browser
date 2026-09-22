/**
 * Unit tests for the pool route handlers: a round-robin command that names
 * the browser that ran it, and clearing a persona's cookie jar.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import { CdpConnectionError } from '../../../../../src/drivers/cdp.ts';
import assert from 'node:assert/strict';
import {
  clearJar,
  exportJar,
  importJar,
  poolCommand,
  queryPersona,
} from '../../../../../src/modules/browsers/http/pool.ts';
import { container } from '../../../../../src/app/container.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-poolcmd';

/** Sends a pool command as `key` and returns the response. */
async function command(key: string, body: object) {
  const res = new FakeResponse();
  await poolCommand(fakeRequest({ key, body }), res);
  return res;
}

describe('poolCommand', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('runs the command on a pool browser and names it in the answer', async () => {
    driveBrowser(B, () => ({ ok: true, data: 1 }), 'k-pool');
    const res = await command('k-pool', { action: 'scroll' });
    assert.deepEqual(res.body, { ok: true, data: 1, _browser: B });
  });

  it('answers 503 when the key has no browsers', async () => {
    const res = await command('k-empty', { action: 'click' });
    assert.deepEqual([res.statusCode, res.body], [503, { error: 'No browsers available in pool' }]);
  });

  it('refuses a server-internal action, just as the browser route does', async () => {
    driveBrowser(B, () => ({ ok: true }), 'k-pool');
    assert.equal((await command('k-pool', { action: 'evaluate' })).statusCode, 403);
  });

  it('answers a page that refused the command as a failed result naming the browser', async () => {
    driveBrowser(B, () => Promise.reject(new Error('Element not found')), 'k-pool');
    const res = await command('k-pool', { action: 'click' });
    assert.deepEqual(
      [res.statusCode, res.body],
      [200, { ok: false, error: 'Element not found', code: 'command_failed', _browser: B }],
    );
  });

  it('answers a lost connection as 504 naming the browser, so the caller knows the outcome is unknown', async () => {
    driveBrowser(B, () => Promise.reject(new CdpConnectionError('CDP connection closed')), 'k-pool');
    const res = await command('k-pool', { action: 'click' });
    assert.deepEqual([res.statusCode, res.body.code, res.body._browser], [504, 'command_outcome_unknown', B]);
  });
});

describe('queryPersona', () => {
  it('reads ?persona= as one id, and nothing when absent', () => {
    assert.equal(queryPersona({ query: { persona: 'p1' } }), 'p1');
    assert.equal(queryPersona({ query: {} }), undefined);
  });

  it('turns a repeated param into an id that cannot match, never the default', () => {
    assert.equal(queryPersona({ query: { persona: ['a', 'b'] } }), 'a,b');
  });
});

describe('clearJar', () => {
  afterEach(() => mock.restoreAll());

  it('clears the default persona’s jar and audits it', () => {
    const res = new FakeResponse();
    clearJar(fakeRequest({ key: 'k-jar' }), res);
    const persona = container.personas.defaultFor('k-jar');
    assert.deepEqual(res.body, { ok: true, persona: persona.id });
    assert.equal(recent({ action: 'cookies.clear' })[0].target_id, persona.id);
  });

  it('answers 404 for a persona the key does not own', () => {
    const res = new FakeResponse();
    clearJar(fakeRequest({ key: 'k-jar', query: { persona: 'p-not-mine' } }), res);
    assert.equal(res.statusCode, 404);
  });

  it('answers 500 when resolving fails without a status', () => {
    mock.method(container.personas, 'resolve', () => {
      throw new Error('store offline');
    });
    const res = new FakeResponse();
    clearJar(fakeRequest(), res);
    assert.deepEqual([res.statusCode, res.body], [500, { error: 'store offline' }]);
  });
});

describe('importJar', () => {
  const cookie = (name: string, extra: object = {}) => ({ name, value: 'v', domain: '.site.test', ...extra });

  it('merges the cookies into the persona’s jar, says how many were kept, and audits it', () => {
    const res = new FakeResponse();
    importJar(
      fakeRequest({
        key: 'k-import',
        body: { cookies: [cookie('sid'), { name: 'broken' }, cookie('old', { expires: 1 })] },
      }),
      res,
    );
    const persona = container.personas.defaultFor('k-import');
    assert.deepEqual(res.body, { ok: true, persona: persona.id, imported: 1, skipped: 2, total: 1 });
    assert.equal(recent({ action: 'cookies.import' })[0].target_id, persona.id);
  });

  it('answers 400 for a body without a cookie list, or with too many', () => {
    const none = new FakeResponse();
    importJar(fakeRequest({ key: 'k-import', body: {} }), none);
    assert.equal(none.statusCode, 400);
    const many = new FakeResponse();
    importJar(fakeRequest({ key: 'k-import', body: { cookies: Array(20_001).fill(cookie('x')) } }), many);
    assert.equal(many.statusCode, 400);
  });

  it('answers 404 for a persona the key does not own', () => {
    const res = new FakeResponse();
    importJar(fakeRequest({ key: 'k-import', query: { persona: 'p-not-mine' }, body: { cookies: [] } }), res);
    assert.equal(res.statusCode, 404);
  });
});

describe('exportJar', () => {
  it('answers the jar as json by default, in Playwright’s shape when asked, and as a cookies.txt download', () => {
    importJar(
      fakeRequest({ key: 'k-export', body: { cookies: [{ name: 'sid', value: 'v', domain: '.site.test' }] } }),
      new FakeResponse(),
    );
    const json = new FakeResponse();
    exportJar(fakeRequest({ key: 'k-export' }), json);
    assert.deepEqual(json.body.cookies, [{ name: 'sid', value: 'v', domain: '.site.test' }]);
    const playwright = new FakeResponse();
    exportJar(fakeRequest({ key: 'k-export', query: { format: 'playwright' } }), playwright);
    assert.equal(playwright.body.cookies[0].expires, -1);
    const text = new FakeResponse();
    exportJar(fakeRequest({ key: 'k-export', query: { format: 'netscape' } }), text);
    assert.match(String(text.ended), /^# Netscape HTTP Cookie File\n\.site\.test\tTRUE/);
    assert.match(text.headers['Content-Disposition'], /attachment; filename="cookies-.*\.txt"/);
  });

  it('answers 400 for a format it does not know', () => {
    const res = new FakeResponse();
    exportJar(fakeRequest({ key: 'k-export', query: { format: 'yaml' } }), res);
    assert.equal(res.statusCode, 400);
  });
});
