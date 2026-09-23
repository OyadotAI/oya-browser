/**
 * Unit tests for the telemetry seams: every event goes out with only the
 * properties its catalog entry names, the few that have a Slack line post it
 * to the right channel with nothing sensitive in it, a seam never waits on
 * anything, and nothing at all leaves when the operator set no destination.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { track, clientOf } from '../../../../src/modules/telemetry/index.ts';
import { forgetIdentifiedForTests } from '../../../../src/modules/telemetry/service.ts';
import { forgetAllForTests, keyLabel } from '../../../../src/modules/telemetry/who.ts';
import { drain, resetForTests } from '../../../../src/platform/analytics.ts';
import { stubFetch, json } from '../../support/http.ts';

/** A key long enough to look like one; no owner is known for it here (no Supabase in tests). */
const KEY = 'oya_k_0123456789abcdef0123456789abcdef';
/** Every seam that takes a key, driven with realistic props. */
const KEYED: Array<() => void> = [
  () => track.browserStarted(KEY, { provider: 'browserbase', persona: true, via: 'mcp' }),
  () => track.browserStopped(KEY, { provider: 'cdp', seconds: 42 }),
  () => track.playbookSaved(KEY, { steps: 14 }),
  () => track.playbookReplayed(KEY, { outcome: 'healed', steps: 14, healed: true }),
  () => track.mcpToolCalled(KEY, { tool: 'click' }),
  () => track.cdpAttached(KEY, { provider: 'steel' }),
  () => track.desktopConnected(KEY, { platform: 'MacIntel', first: true, version: '1.0.115' }),
  () => track.desktopUpdated(KEY, { platform: 'MacIntel', from: '1.0.114', to: '1.0.115' }),
  () => track.personaCreated(KEY, { has_proxy: false }),
  () => track.serverError(KEY, { ref: '9f2b7c1d', method: 'POST', route: '/browsers/:id/playbooks' }),
];

/** Turns every destination on, pointed at a fake network. */
function allOn() {
  process.env.POSTHOG_KEY = 'phc_test';
  process.env.POSTHOG_HOST = 'https://ph.example.test';
  process.env.SLACK_OPS_WEBHOOK_SIGNUPS = 'https://hooks.example.test/signups';
  process.env.SLACK_OPS_WEBHOOK_EVENTS = 'https://hooks.example.test/events';
}

/** Turns everything off. */
function allOff() {
  for (const name of ['POSTHOG_KEY', 'POSTHOG_HOST', 'SLACK_OPS_WEBHOOK_SIGNUPS', 'SLACK_OPS_WEBHOOK_EVENTS'])
    delete process.env[name];
}

/** Lets fire-and-forget work settle. */
const settle = () => new Promise((r) => setTimeout(r, 5));

describe('track', () => {
  beforeEach(() => {
    allOff();
    resetForTests();
    forgetAllForTests();
    forgetIdentifiedForTests();
  });
  afterEach(() => {
    mock.restoreAll();
    allOff();
  });

  it('sends nothing anywhere when no destination is configured', async () => {
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' });
    for (const seam of KEYED) seam();
    await settle();
    await drain();
    assert.equal(calls.length, 0);
  });

  it('never puts a key, a URL, an email outside the allowed lines, or free text into what leaves', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' });
    track.apiKeyCreated({ id: 'u-1', email: 'ana@example.com' }, '7c1e9a02-aaaa-bbbb');
    for (const seam of KEYED) seam();
    await settle();
    await drain();
    const everything = calls.map((c) => c.init.body).join('\n');
    assert.ok(!everything.includes(KEY), 'the API key must never leave');
    assert.ok(!/[A-Za-z0-9_-]{32,}/.test(everything.replace(/phc_test/g, '')), 'nothing key-shaped leaves');
    const slackTexts = calls.filter((c) => c.url.includes('hooks.')).map((c) => JSON.parse(c.init.body).text);
    for (const text of slackTexts) assert.ok(!/:\/\//.test(text), `no URL in a Slack line: ${text}`);
    const posthog = calls.filter((c) => c.url.includes('/batch/'));
    const events = posthog.flatMap((c) => JSON.parse(c.init.body).batch);
    for (const e of events) assert.ok(!JSON.stringify(e.properties).includes('://'), e.event);
  });

  it('posts each event with a Slack line to its channel, and the rest to PostHog only', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' });
    track.apiKeyCreated({ id: 'u-1', email: 'ana@example.com' }, '7c1e9a02-aaaa');
    track.desktopConnected(KEY, { platform: 'MacIntel', first: true, version: '1.0.115' });
    track.desktopConnected(KEY, { platform: 'MacIntel', first: false, version: '1.0.115' });
    track.downloadServed('dl-1', { platform: 'mac', version: '1.0.115', file_type: 'installer', via: 'web' });
    track.downloadServed('dl-1', { platform: 'mac', version: '1.0.116', file_type: 'update', via: 'updater' });
    track.updateChecked('dl-1', { platform: 'mac', from_version: '1.0.115' });
    track.playbookSaved(KEY, { steps: 1 });
    track.browserStarted(KEY, { provider: 'cdp', persona: false, via: 'rest' });
    track.serverError(null, { ref: '9f2b7c1d', method: 'GET', route: 'middleware' });
    await settle();
    const lines = calls
      .filter((c) => c.url.includes('hooks.'))
      .map((c) => [c.url.split('/').at(-1), JSON.parse(c.init.body).text]);
    const label = keyLabel(KEY);
    // Keyed events look their owner up off the request path, so they land a tick later: order is not the rule.
    assert.deepEqual(
      lines.sort(),
      [
        ['events', `💾 Playbook saved: ${label} (1 step)`],
        ['events', '⚠️ Server error: GET middleware (ref 9f2b7c1d)'],
        ['signups', '🎉 New signup: ana@example.com'],
        ['signups', '🔑 API key created: ana@example.com (project 7c1e9a02)'],
        ['signups', `🖥️ Desktop connected: ${label} (MacIntel)`],
        ['signups', '⬇️ Desktop downloaded: mac 1.0.115'],
      ].sort(),
    );
  });

  it('identifies a person by email in PostHog, and marks a bare key as no person', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' });
    track.playbookSaved(KEY, { steps: 3 });
    await settle();
    await drain();
    const batch = calls.filter((c) => c.url.includes('/batch/')).flatMap((c) => JSON.parse(c.init.body).batch);
    const identify = batch.find((e) => e.event === '$identify');
    assert.deepEqual([identify.distinct_id, identify.properties.$set], ['u-1', { email: 'ana@example.com' }]);
    const keyed = batch.find((e) => e.event === 'playbook_saved');
    assert.equal(keyed.properties.$process_person_profile, false);
    assert.notEqual(keyed.distinct_id, KEY);
  });

  it('returns at once from every seam even when the network never answers', () => {
    allOn();
    stubFetch(() => new Promise(() => {}));
    const started = Date.now();
    track.accountSignedUp({ id: 'u-1' });
    for (const seam of KEYED) seam();
    assert.ok(Date.now() - started < 50);
  });

  it('reads which client a start came from off its header, and calls anything else rest', () => {
    assert.equal(clientOf({ 'x-oya-client': 'mcp' }), 'mcp');
    assert.equal(clientOf({ 'x-oya-client': 'console' }), 'console');
    assert.equal(clientOf({ 'x-oya-client': 'curl' }), 'rest');
    assert.equal(clientOf({}), 'rest');
  });
});
