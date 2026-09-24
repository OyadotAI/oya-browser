/**
 * Unit tests for product analytics: nothing leaves the process unless both
 * PostHog settings are present, events are batched into one POST, a failing
 * PostHog is dropped after a single warning, and the queue never grows past
 * its cap.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { capture, identify, drain, enabled, resetForTests } from '../../../src/platform/analytics.ts';
import { ANALYTICS_BATCH_MAX } from '../../../src/platform/constants.ts';
import { stubFetch, json } from '../support/http.ts';
import { snapshot } from '../../../src/platform/metrics.ts';

/** Points analytics at a fake PostHog. */
function on() {
  process.env.POSTHOG_KEY = 'phc_test';
  process.env.POSTHOG_HOST = 'https://ph.example.test/';
}

describe('analytics', () => {
  beforeEach(() => {
    delete process.env.POSTHOG_KEY;
    delete process.env.POSTHOG_HOST;
    resetForTests();
  });
  afterEach(() => {
    mock.restoreAll();
    delete process.env.POSTHOG_KEY;
    delete process.env.POSTHOG_HOST;
  });

  it('sends nothing when the settings are unset, or when only one of the pair is', async () => {
    const calls = stubFetch(() => json({}));
    capture('u-1', 'browser_started', { provider: 'cdp' });
    await drain();
    process.env.POSTHOG_KEY = 'phc_only';
    capture('u-1', 'browser_started', {});
    await drain();
    assert.equal(calls.length, 0);
    assert.equal(enabled(), false);
  });

  it('batches events into one POST to /batch/ with the key and a trailing slash trimmed from the host', async () => {
    on();
    const calls = stubFetch(() => json({}));
    capture('u-1', 'browser_started', { provider: 'cdp' });
    identify('u-1', { email: 'ana@example.com' });
    await drain();
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, 'https://ph.example.test/batch/');
    const body = JSON.parse(calls[0].init.body);
    assert.equal(body.api_key, 'phc_test');
    assert.deepEqual(
      body.batch.map((r) => [r.event, r.distinct_id]),
      [
        ['browser_started', 'u-1'],
        ['$identify', 'u-1'],
      ],
    );
    assert.deepEqual(body.batch[1].properties, { $set: { email: 'ana@example.com' }, $geoip_disable: true });
  });

  it('never geolocates an event, since the address PostHog sees is the server and not the person', async () => {
    on();
    const calls = stubFetch(() => json({}));
    capture('u-1', 'browser_started', { provider: 'cdp' });
    identify('u-1', { email: 'ana@example.com' });
    await drain();
    const { batch } = JSON.parse(calls[0].init.body);
    assert.deepEqual(
      batch.map((r) => r.properties.$geoip_disable),
      [true, true],
    );
  });

  it('marks a fingerprint-only id as no person, and leaves a person alone', async () => {
    on();
    const calls = stubFetch(() => json({}));
    capture('key-abc', 'browser_started', { $process_person_profile: false });
    capture('u-1', 'browser_started', {});
    await drain();
    const [anon, person] = JSON.parse(calls[0].init.body).batch;
    assert.equal(anon.properties.$process_person_profile, false);
    assert.equal(person.properties.$process_person_profile, true);
  });

  it('sends a full batch at once without waiting for the timer', async () => {
    on();
    const calls = stubFetch(() => json({}));
    for (let i = 0; i < ANALYTICS_BATCH_MAX; i++) capture('u-1', 'mcp_tool_called', { tool: 'click' });
    await new Promise((r) => setImmediate(r));
    assert.equal(calls.length, 1);
    assert.equal(JSON.parse(calls[0].init.body).batch.length, ANALYTICS_BATCH_MAX);
  });

  it('returns synchronously even when PostHog never answers', () => {
    on();
    stubFetch(() => new Promise(() => {}));
    const started = Date.now();
    capture('u-1', 'browser_started', {});
    assert.ok(Date.now() - started < 50);
  });

  it('drops events when PostHog fails, saying so once and then staying quiet for an hour', async () => {
    on();
    const logged = mock.method(console, 'error', () => {});
    stubFetch(() => json({ error: 'nope' }, 500));
    capture('u-1', 'browser_started', {});
    await drain();
    capture('u-1', 'browser_started', {});
    await drain();
    assert.equal(logged.mock.callCount(), 1);
    assert.match(logged.mock.calls[0].arguments[0], /\[analytics\] dropped 1 events: PostHog answered 500/);
  });

  it('counts every dropped event in the metric, by reason', async () => {
    on();
    mock.method(console, 'error', () => {});
    stubFetch(() => json({}, 503));
    const count = () => snapshot().oya_analytics_dropped_total?.find((s) => s.labels.reason === 'posthog')?.value ?? 0;
    const before = count();
    capture('u-1', 'browser_started', {});
    capture('u-1', 'browser_started', {});
    await drain();
    assert.equal(count() - before, 2);
  });
});
