/**
 * Unit tests for the external CAPTCHA solver: its settings, the create-then-poll
 * exchange with CapSolver through a stubbed fetch, and every way a solve fails.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { CAPTCHA_POLL_MS } from '../../../../src/modules/challenges/constants.ts';
import { Status } from '../../../../src/platform/http-status.ts';
import { advance, json, stubFetch } from '../../support/http.ts';

// The poll loop sleeps through timers/promises, whose binding is fixed when
// first imported: importing with the clock mocked lets mock.timers drive it.
mock.timers.enable({ apis: ['setTimeout'] });
const { isConfigured, solveExternally } = await import('../../../../src/modules/challenges/captcha-solver.ts');
mock.timers.reset();

/** Solver settings with a key. */
const ENV = { OYA_CAPTCHA_API_KEY: 'cap-key' };
/** The page being solved. */
const PAGE = 'https://site.example/login';

/** Answers createTask with a task id, then each poll with the next of `polls`. */
function solver(polls: any[]) {
  return stubFetch((url) => (url.endsWith('/createTask') ? json({ taskId: 't-1' }) : json(polls.shift())));
}

describe('isConfigured', () => {
  it('is true only with an API key', () => {
    assert.equal(isConfigured(ENV), true);
    assert.equal(isConfigured({}), false);
  });
});

describe('solveExternally', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('creates a task for the challenge and polls until the token is ready', async () => {
    const calls = solver([{ status: 'processing' }, { status: 'ready', solution: { token: 'tok' } }]);
    const pending = solveExternally('turnstile', 'site-key', PAGE, ENV);
    await advance(CAPTCHA_POLL_MS, 2);
    assert.deepEqual(await pending, { token: 'tok' });
    assert.deepEqual(JSON.parse(calls[0].init.body), {
      clientKey: 'cap-key',
      task: { type: 'AntiTurnstileTaskProxyLess', websiteURL: PAGE, websiteKey: 'site-key' },
    });
    assert.deepEqual(JSON.parse(calls[1].init.body), { clientKey: 'cap-key', taskId: 't-1' });
  });

  it('maps each challenge type to its task, reCAPTCHA v2 by default', async () => {
    for (const [type, task] of [
      ['hcaptcha', 'HCaptchaTaskProxyLess'],
      ['recaptcha_v3', 'ReCaptchaV3TaskProxyLess'],
      ['recaptcha_v2', 'ReCaptchaV2TaskProxyLess'],
      ['toString', 'ReCaptchaV2TaskProxyLess'],
    ]) {
      const calls = solver([{ status: 'ready', solution: { gRecaptchaResponse: 'g' } }]);
      const pending = solveExternally(type, 'k', PAGE, ENV);
      await advance(CAPTCHA_POLL_MS);
      assert.deepEqual(await pending, { token: 'g' });
      assert.equal(JSON.parse(calls[0].init.body).task.type, task, type);
      mock.restoreAll();
    }
  });

  it('refuses an unknown provider, a missing key and a missing sitekey before calling out', async () => {
    const calls = solver([]);
    await assert.rejects(solveExternally('turnstile', 'k', PAGE, { ...ENV, OYA_CAPTCHA_PROVIDER: 'nope' }), {
      status: Status.BAD_REQUEST,
      message: 'Unknown CAPTCHA provider: nope',
    });
    await assert.rejects(solveExternally('turnstile', 'k', PAGE, {}), { status: Status.CONFLICT });
    await assert.rejects(solveExternally('turnstile', null, PAGE, ENV), {
      status: Status.UNPROCESSABLE,
      message: 'No sitekey found for the challenge',
    });
    assert.equal(calls.length, 0);
  });

  it('fails when the solver returns no task id', async () => {
    stubFetch(() => json({}));
    await assert.rejects(solveExternally('turnstile', 'k', PAGE, ENV), {
      message: 'capsolver did not return a task id',
    });
  });

  it('fails naming the provider when it answers with an error status', async () => {
    stubFetch(() => json({}, 500));
    await assert.rejects(solveExternally('turnstile', 'k', PAGE, ENV), { message: 'capsolver returned 500' });
  });

  it('passes on the solver’s own failure reason', async () => {
    solver([{ status: 'failed', errorDescription: 'bad sitekey' }]);
    const pending = assert.rejects(solveExternally('turnstile', 'k', PAGE, ENV), { message: 'bad sitekey' });
    await advance(CAPTCHA_POLL_MS);
    await pending;
  });

  it('times out with 504 after the configured window', async () => {
    solver(Array.from({ length: 5 }, () => ({ status: 'processing' })));
    const env = { ...ENV, OYA_CAPTCHA_TIMEOUT_MS: String(CAPTCHA_POLL_MS * 2) };
    const pending = assert.rejects(solveExternally('turnstile', 'k', PAGE, env), {
      status: Status.GATEWAY_TIMEOUT,
      message: 'CAPTCHA solve timed out',
    });
    await advance(CAPTCHA_POLL_MS, 2);
    await pending;
  });
});
