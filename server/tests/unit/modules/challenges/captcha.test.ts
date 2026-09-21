/**
 * Unit tests for CAPTCHA handling through a scripted page: detection, leaving
 * provider-solved challenges alone, solving externally and placing the token,
 * and reporting (never throwing) every failure.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { CAPTCHA_POLL_MS } from '../../../../src/modules/challenges/constants.ts';
import { advance, json, stubFetch } from '../../support/http.ts';
import { scriptedPage } from '../../support/challenges.ts';

// The solver sleeps through timers/promises, whose binding is fixed when first
// imported: importing with the clock mocked lets mock.timers drive it.
mock.timers.enable({ apis: ['setTimeout'] });
const { handle, DETECT_JS, applyTokenJS } = await import('../../../../src/modules/challenges/captcha.ts');
mock.timers.reset();

/** A Turnstile challenge, as DETECT_JS reports it. */
const FOUND = { present: true, type: 'turnstile', sitekey: 'sk', url: 'https://site.example/', invisible: false };
/** Solver settings with a key. */
const ENV = { OYA_CAPTCHA_API_KEY: 'cap-key' };

describe('captcha.handle', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('reports nothing when the page has no challenge', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [{ present: false }]]]);
    assert.deepEqual(await handle(evaluate, { env: ENV }), { present: false, solved: false, method: 'none' });
  });

  it('leaves a challenge to a provider that solves natively', async () => {
    const calls = stubFetch(() => json({}));
    const evaluate = scriptedPage([[DETECT_JS, [FOUND]]]);
    const result = await handle(evaluate, { providerSolves: true, env: ENV });
    assert.equal(result.method, 'provider');
    assert.match(result.note, /solves natively/);
    assert.equal(calls.length, 0, 'solving twice would pay twice');
  });

  it('only detects when asked not to solve', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [FOUND]]]);
    assert.deepEqual(await handle(evaluate, { solve: false, env: ENV }), { ...FOUND, solved: false, method: 'none' });
  });

  it('says how to fix it when no solver is configured', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [FOUND]]]);
    const result = await handle(evaluate, { env: {} });
    assert.equal(result.solved, false);
    assert.match(result.error, /OYA_CAPTCHA_API_KEY/);
  });

  it('solves externally and places the token in the page', async () => {
    stubFetch((url) =>
      url.endsWith('/createTask') ? json({ taskId: 't' }) : json({ status: 'ready', solution: { token: 'tok' } }),
    );
    const evaluate = scriptedPage([
      [DETECT_JS, [FOUND]],
      [applyTokenJS('turnstile', 'tok'), [{ placed: true }]],
    ]);
    const pending = handle(evaluate, { env: ENV });
    await advance(CAPTCHA_POLL_MS);
    assert.deepEqual(await pending, { ...FOUND, solved: true, method: 'solver' });
  });

  it('does not call a token nobody accepted solved', async () => {
    stubFetch((url) =>
      url.endsWith('/createTask') ? json({ taskId: 't' }) : json({ status: 'ready', solution: { token: 'tok' } }),
    );
    const evaluate = scriptedPage([
      [DETECT_JS, [FOUND]],
      [applyTokenJS('turnstile', 'tok'), [{ placed: false }]],
    ]);
    const pending = handle(evaluate, { env: ENV });
    await advance(CAPTCHA_POLL_MS);
    const result = await pending;
    assert.equal(result.solved, false);
    assert.equal(result.error, 'Solved, but no field accepted the token');
  });

  it('reports a solver failure instead of throwing', async () => {
    stubFetch(() => json({}, 500));
    const evaluate = scriptedPage([[DETECT_JS, [FOUND]]]);
    const result = await handle(evaluate, { env: ENV });
    assert.deepEqual(
      { solved: result.solved, method: result.method, error: result.error },
      {
        solved: false,
        method: 'solver',
        error: 'capsolver returned 500',
      },
    );
  });
});

describe('applyTokenJS', () => {
  it('embeds the token and type as JSON strings', () => {
    const script = applyTokenJS('hcaptcha', 'a"b');
    assert.ok(script.includes('const token = "a\\"b"'));
    assert.ok(script.includes('const type = "hcaptcha"'));
  });
});
