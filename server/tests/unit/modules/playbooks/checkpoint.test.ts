/**
 * Unit tests for the checkpoint a run takes between page-changing steps: with
 * nothing on the page it does nothing; a CAPTCHA, sign-in or MFA prompt it
 * cannot clear parks the run on a person, in that order.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { checkpointFor } = await import('../../../../src/modules/playbooks/checkpoint.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const KEY = 'checkpoint-key';
const BROWSER = 'b-checkpoint';
let detected: any;
let browser;
let events: any[];

describe('checkpointFor', () => {
  beforeEach(() => {
    detected = { present: false };
    events = [];
    browser = scriptedBrowser(BROWSER, KEY, (action) =>
      action === 'evaluate_raw' ? { ok: true, data: { result: detected } } : { ok: true },
    );
    mock.method(control(), 'emit', async (_key, type, _id, detail) => void events.push({ type, detail }));
    mock.method(console, 'log', () => {});
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
  });

  it('does nothing when the page has no challenge', async () => {
    const requestHuman = mock.fn();
    await checkpointFor(KEY, BROWSER, requestHuman)();
    assert.equal(requestHuman.mock.callCount(), 0);
    assert.equal(
      browser.calls.filter((c) => c.action === 'evaluate_raw').length,
      3,
      'CAPTCHA, sign-in and MFA were each checked',
    );
  });

  it('parks on a person for a CAPTCHA, a sign-in and an MFA prompt it cannot clear, in that order', async () => {
    detected = { present: true, type: 'recaptcha' };
    const requestHuman = mock.fn(async () => 'done');
    await checkpointFor(KEY, BROWSER, requestHuman)();
    const asks = requestHuman.mock.calls.map((c) => c.arguments[0]);
    assert.deepEqual(
      asks.map((a) => a.reason),
      ['captcha', 'login', 'mfa'],
    );
    assert.ok(asks.every((a) => a.liveViewUrl === `/dashboard/?browser=${BROWSER}`));
    assert.equal(events.find((e) => e.type === 'login.failed').detail.personaId, 'p-1');
  });

  it('leaves a CAPTCHA to a provider that solves it natively', async () => {
    registry.get(BROWSER).provider = 'anchor';
    detected = { present: true, type: 'recaptcha' };
    const requestHuman = mock.fn(async () => 'done');
    await checkpointFor(KEY, BROWSER, requestHuman)();
    assert.ok(!requestHuman.mock.calls.some((c) => c.arguments[0].reason === 'captcha'));
  });

  it('types an MFA code a person replies with', async () => {
    detected = { present: true };
    const requestHuman = mock.fn(async (ask) => (ask.reason === 'mfa' ? 'the code is 482913' : 'done'));
    await checkpointFor(KEY, BROWSER, requestHuman)();
    const expressions = browser.calls.filter((c) => c.action === 'evaluate_raw').map((c) => c.params.expression);
    assert.ok(expressions.some((e) => e.includes('482913')));
  });

  it('carries on when the page cannot be checked', async () => {
    browser.disconnect();
    browser = scriptedBrowser(BROWSER, KEY, () => {
      throw new Error('detached');
    });
    const requestHuman = mock.fn();
    await checkpointFor(KEY, BROWSER, requestHuman)();
    assert.equal(requestHuman.mock.callCount(), 0);
  });

  it('uses the key’s default persona for a browser that is gone', async () => {
    browser.disconnect();
    const requestHuman = mock.fn();
    await checkpointFor(KEY, 'b-gone', requestHuman)();
    assert.equal(requestHuman.mock.callCount(), 0);
  });
});
