/**
 * Unit tests for the CAPTCHA and MFA routes, driven through a browser whose
 * page scripts answer from the test: nothing present, a provider that solves
 * natively, a solve the caller declined, and an MFA prompt with no factor.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { completeMfa, solveCaptcha } from '../../../../../src/modules/browsers/http/challenges.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-challenge';

/** A browser whose page scripts all answer `found`. */
const pageFinds = (found: object | null, provider = 'cdp') => {
  driveBrowser(B, () => ({ ok: true, data: { result: found } }));
  registry.get(B).provider = provider;
};

/** Calls `handler` for B with `body` and returns the response. */
async function call(handler, body: object = {}) {
  const res = new FakeResponse();
  await handler(fakeRequest({ params: { browserId: B }, body }), res);
  return res;
}

describe('solveCaptcha', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('reports no CAPTCHA and audits nothing when the page has none', async () => {
    pageFinds({ present: false });
    const before = recent({ action: 'captcha.handle' }).length;
    const res = await call(solveCaptcha);
    assert.deepEqual(res.body, { present: false, solved: false, method: 'none' });
    assert.equal(recent({ action: 'captcha.handle' }).length, before);
  });

  it('leaves a CAPTCHA to a provider that solves natively', async () => {
    pageFinds({ present: true, type: 'recaptcha' }, 'browserbase');
    const res = await call(solveCaptcha);
    assert.equal(res.body.method, 'provider');
  });

  it('does not solve when the caller says not to, and audits the unsolved CAPTCHA', async () => {
    pageFinds({ present: true, type: 'hcaptcha' });
    const res = await call(solveCaptcha, { solve: false });
    assert.deepEqual([res.body.solved, res.body.method], [false, 'none']);
    const [row] = recent({ action: 'captcha.handle' });
    assert.deepEqual([row.outcome, row.meta.type], ['error', 'hcaptcha']);
  });
});

describe('completeMfa', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('reports no prompt when the page has none', async () => {
    pageFinds(null);
    const res = await call(completeMfa);
    assert.deepEqual(res.body, { present: false, completed: false, method: 'none' });
  });

  it('hands a prompt with no configured factor to a person, via the live view', async () => {
    pageFinds({ present: true });
    registry.updateUrl(B, 'https://accounts.example.com/login');
    const res = await call(completeMfa);
    assert.equal(res.body.method, 'handoff');
    assert.equal(res.body.liveViewUrl, `/dashboard/?browser=${B}`);
    assert.equal(recent({ action: 'mfa.complete' })[0].outcome, 'error');
  });
});
