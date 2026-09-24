/**
 * Unit tests for answering a start: the event it sends marks the key's first
 * browser, once, and a borrowed browser never counts as one.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';
import { stubFetch, json } from '../../../support/http.ts';
import { FakeResponse, fakeRequest } from '../../../support/browsers.ts';

ownDataDir('oya-start-reply-');
const { started } = await import('../../../../../src/modules/browsers/lifecycle/start-reply.ts');
const { drain, resetForTests } = await import('../../../../../src/platform/analytics.ts');

/** Answers one start for `key` and returns what PostHog was told `first` was. */
async function firstOf(key: string, body: object = {}) {
  const calls = stubFetch(() => json({}));
  const start = { req: fakeRequest({ key }), res: new FakeResponse(), key, wanted: 'oya', persona: { id: 'p' } };
  started(start, { id: 'b-1', provider: 'oya', ...body });
  await new Promise((r) => setTimeout(r, 5));
  await drain();
  mock.restoreAll();
  const batch = calls.flatMap((c) => JSON.parse(c.init.body).batch);
  return batch.find((e) => e.event === 'browser_started').properties.first;
}

describe('started', () => {
  beforeEach(() => {
    process.env.POSTHOG_KEY = 'phc_test';
    process.env.POSTHOG_HOST = 'https://ph.example.test';
    resetForTests();
  });
  afterEach(() => {
    delete process.env.POSTHOG_KEY;
    delete process.env.POSTHOG_HOST;
  });

  it('marks a key’s first browser as first, and none after it', async () => {
    assert.equal(await firstOf('first-key'), true);
    assert.equal(await firstOf('first-key'), false);
  });

  it('never counts a borrowed browser as the first', async () => {
    assert.equal(await firstOf('borrow-key', { reused: true }), false);
    assert.equal(await firstOf('borrow-key'), true);
  });
});
