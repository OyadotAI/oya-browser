/**
 * Unit tests for the telemetry seams: every event goes out with only the
 * properties its catalog entry names, the few that have a Slack line post it
 * to the right channel with nothing sensitive in it, a seam never waits on
 * anything, and nothing at all leaves when the operator set no destination.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { track, clientOf } from '../../../../src/modules/telemetry/index.ts';
import { forgetAllForTests, keyLabel } from '../../../../src/modules/telemetry/who.ts';
import { drain, resetForTests } from '../../../../src/platform/analytics.ts';
import { stubFetch, json } from '../../support/http.ts';

/** A key long enough to look like one; no owner is known for it here (no database in tests). */
const KEY = 'oya_k_0123456789abcdef0123456789abcdef';
/** Every seam that takes a key, driven with realistic props. */
const KEYED: Array<() => void> = [
  () => track.browserStarted(KEY, { provider: 'browserbase', persona: true, via: 'mcp', first: true }),
  () => track.browserStopped(KEY, { provider: 'oya-desktop', seconds: 3960 }),
  () => track.playbookSaved(KEY, { steps: 14 }),
  () => track.playbookReplayed(KEY, { outcome: 'healed', steps: 14, healed: true }),
  () => track.mcpToolCalled(KEY, { tool: 'click' }),
  () => track.cdpAttached(KEY, { provider: 'oya-cloud' }),
  () => track.desktopConnected(KEY, { platform: 'MacIntel', first: true, version: '1.0.115' }),
  () => track.desktopUpdated(KEY, { platform: 'MacIntel', from: '1.0.114', to: '1.0.115' }),
  () => track.personaCreated(KEY, { has_proxy: false }),
  () => track.serverError(KEY, { ref: '9f2b7c1d', method: 'POST', route: '/browsers/:id/playbooks' }),
  () => track.runFailed(KEY, { run_id: 'run_1', error: 'Timed out' }),
];

/** Business alerts, including agent onboarding and both payment outcomes. */
const BUSINESS: Array<() => void> = [
  () => track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' }, 'google'),
  () => track.agentSignedUp(KEY, 'ana@example.com'),
  () => track.agentKeyClaimed({ id: 'u-1', email: 'ana@example.com' }),
  () => track.apiKeyCreated({ id: 'u-1', email: 'ana@example.com' }, '7c1e9a02-aaaa'),
  () =>
    track.paymentReceived(
      { id: 'u-1', email: 'ana@example.com' },
      { amount_cents: 9900, currency: 'usd', reason: 'subscription_cycle' },
    ),
  () =>
    track.paymentFailed({ id: 'u-1', email: 'ana@example.com' }, { amount_cents: 2000, currency: 'eur', attempt: 2 }),
];

/** Nonbusiness events include every formerly noisy card and signup line. */
function noise() {
  for (const seam of KEYED) seam();
  track.agentCloudRefused(KEY, 'oya-cloud');
  track.downloadServed('dl-1', {
    platform: 'mac',
    version: '1.0.115',
    file_type: 'installer',
    via: 'web',
    source: 'direct',
    client: '',
  });
  track.updateChecked('dl-1', { platform: 'mac', from_version: '1.0.115' });
  track.installPinged('i-1', { version: '1.0.135', browsers: 9, peak_cloud: 9, licensed: false });
}

/** Turns every destination on, pointed at a fake network. */
function allOn() {
  process.env.POSTHOG_KEY = 'phc_test';
  process.env.POSTHOG_HOST = 'https://ph.example.test';
  process.env.SLACK_OPS_WEBHOOK_SIGNUPS = 'https://hooks.example.test/signups';
  process.env.SLACK_OPS_WEBHOOK_EVENTS = 'https://hooks.example.test/events';
  process.env.SLACK_OPS_WEBHOOK_PRODUCT = 'https://hooks.example.test/product';
}

/** Turns everything off. */
function allOff() {
  for (const name of [
    'POSTHOG_KEY',
    'POSTHOG_HOST',
    'SLACK_OPS_WEBHOOK_SIGNUPS',
    'SLACK_OPS_WEBHOOK_EVENTS',
    'SLACK_OPS_WEBHOOK_PRODUCT',
  ])
    delete process.env[name];
}

/** Lets fire-and-forget work settle. */
const settle = () => new Promise((r) => setTimeout(r, 5));

describe('track', () => {
  beforeEach(() => {
    allOff();
    resetForTests();
    forgetAllForTests();
  });
  afterEach(() => {
    mock.restoreAll();
    allOff();
  });

  it('sends nothing anywhere when no destination is configured', async () => {
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' }, 'email');
    for (const seam of KEYED) seam();
    await settle();
    await drain();
    assert.equal(calls.length, 0);
  });

  it('never puts a key, a URL, an email outside the allowed lines, or free text into what leaves', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' }, 'email');
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
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' }, 'email');
    track.apiKeyCreated({ id: 'u-1', email: 'ana@example.com' }, '7c1e9a02-aaaa');
    track.desktopConnected(KEY, { platform: 'MacIntel', first: true, version: '1.0.115' });
    track.desktopConnected(KEY, { platform: 'MacIntel', first: false, version: '1.0.115' });
    track.downloadServed('dl-1', {
      platform: 'mac',
      version: '1.0.115',
      file_type: 'installer',
      via: 'web',
      source: 'facebook ad',
      client: '',
    });
    track.downloadServed('dl-2', {
      platform: 'windows',
      version: '1.0.115',
      file_type: 'installer',
      via: 'web',
      source: 'direct',
      client: 'curl',
    });
    track.downloadServed('dl-1', {
      platform: 'mac',
      version: '1.0.116',
      file_type: 'update',
      via: 'updater',
      source: 'direct',
      client: '',
    });
    track.updateChecked('dl-1', { platform: 'mac', from_version: '1.0.115' });
    track.installPinged('i-1', { version: '1.0.135', browsers: 9, peak_cloud: 9, licensed: false });
    track.paymentReceived(
      { id: 'u-2', email: 'bo@example.com' },
      { amount_cents: 9900, currency: 'usd', reason: 'subscription_cycle' },
    );
    track.paymentFailed({ id: 'u-2', email: 'bo@example.com' }, { amount_cents: 2000, currency: 'eur', attempt: 2 });
    track.installPinged('i-2', { version: '1.0.135', browsers: 9, peak_cloud: 9, licensed: true });
    track.installPinged('i-3', { version: '1.0.135', browsers: 3, peak_cloud: 3, licensed: false });
    track.playbookSaved(KEY, { steps: 1 });
    track.browserStarted(KEY, { provider: 'cdp', persona: false, via: 'rest', first: false });
    track.serverError(null, { ref: '9f2b7c1d', method: 'GET', route: 'middleware' });
    track.runFailed(KEY, { run_id: 'run_1', error: 'Timed out\n    at step 3' });
    await settle();
    const lines = calls
      .filter((c) => c.url.includes('hooks.') && !c.url.endsWith('/product'))
      .map((c) => [c.url.split('/').at(-1), JSON.parse(c.init.body).text]);
    const label = keyLabel(KEY);
    // Keyed events look their owner up off the request path, so they land a tick later: order is not the rule.
    assert.deepEqual(
      lines.sort(),
      [
        ['events', `💾 Playbook saved: ${label} (1 step)`],
        ['events', `❌ Run failed: Timed out (run_1) for ${label}`],
        ['events', '⚠️ Server error: GET middleware (ref 9f2b7c1d)'],
        ['signups', '🎉 New signup: ana@example.com'],
        ['signups', '🔑 API key created: ana@example.com (project 7c1e9a02)'],
        ['events', `🖥️ Desktop connected: ${label} (MacIntel)`],
        ['events', '⬇️ Desktop downloaded: mac 1.0.115 · from facebook ad'],
        ['events', '⬇️ Desktop downloaded: windows 1.0.115 · from direct · 🤖 not a browser: curl'],
        ['events', '🏢 Self-hosted server install-i-1 ran 9 cloud browsers at once (v1.0.135), unlicensed'],
        ['signups', '💰 Payment received: $99.00 from bo@example.com (renewal)'],
        ['signups', '⚠️ Payment failed: 20.00 EUR from bo@example.com (attempt 2)'],
      ].sort(),
    );
  });

  it('posts every business alert as a signup line and product card, including payment details', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    for (const seam of BUSINESS) seam();
    await settle();
    const cards = calls.filter((c) => c.url.endsWith('/product')).map((c) => JSON.parse(c.init.body).text);
    assert.deepEqual(cards.map((t) => t.split('\n')[1]).sort(), [
      '📌 Event: account_signed_up',
      '📌 Event: agent_key_claimed',
      '📌 Event: agent_signed_up',
      '📌 Event: api_key_created',
      '📌 Event: payment_failed',
      '📌 Event: payment_received',
    ]);
    assert.equal(calls.filter((c) => c.url.endsWith('/signups')).length, BUSINESS.length);
    assert.ok(cards.some((t) => t.includes('👤 User: ana@example.com') && t.includes('🔐 Method: google')));
    assert.ok(cards.some((t) => /payment_received/.test(t) && /\$99\.00/.test(t) && /renewal/.test(t)));
    assert.ok(cards.some((t) => /payment_failed/.test(t) && /20\.00 EUR/.test(t) && /Attempt: 2/.test(t)));
    assert.ok(!JSON.stringify(calls).includes(KEY));
  });

  it('keeps all nonbusiness events out of both business feeds while retaining analytics and operational lines', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    noise();
    await settle();
    await drain();
    assert.equal(calls.filter((c) => /\/(signups|product)$/.test(c.url)).length, 0);
    const lines = calls.filter((c) => c.url.endsWith('/events')).map((c) => JSON.parse(c.init.body).text);
    for (const prefix of [
      '🖥️ Desktop connected:',
      '⬇️ Desktop downloaded:',
      '🏢 Self-hosted server',
      '🔌 CDP attached:',
    ])
      assert.ok(
        lines.some((line) => line.startsWith(prefix)),
        prefix,
      );
    const events = calls.filter((c) => c.url.includes('/batch/')).flatMap((c) => JSON.parse(c.init.body).batch);
    assert.deepEqual(events.map((e) => e.event).sort(), [
      'agent_cloud_refused',
      'browser_started',
      'browser_stopped',
      'cdp_attached',
      'desktop_connected',
      'desktop_updated',
      'download_served',
      'install_pinged',
      'mcp_tool_called',
      'persona_created',
      'playbook_replayed',
      'playbook_saved',
      'run_failed',
      'server_error',
      'update_checked',
    ]);
    const stopped = events.find((e) => e.event === 'browser_stopped');
    assert.equal(stopped.properties.provider, 'oya-desktop');
    assert.equal(stopped.properties.seconds, 3960);
    assert.equal(events.find((e) => e.event === 'cdp_attached').properties.provider, 'oya-cloud');
  });

  for (const mode of ['product alone', 'shared business webhook']) {
    it(`suppresses nonbusiness alerts with ${mode} configured`, async () => {
      process.env.SLACK_OPS_WEBHOOK_PRODUCT = 'https://hooks.example.test/business';
      if (mode === 'shared business webhook')
        process.env.SLACK_OPS_WEBHOOK_SIGNUPS = process.env.SLACK_OPS_WEBHOOK_PRODUCT;
      const calls = stubFetch(() => json({}));
      noise();
      await settle();
      assert.equal(calls.length, 0);
      track.accountSignedUp({ id: 'u-1' }, 'email');
      assert.ok(calls.length > 0, 'business alerts still reach the configured destination');
    });
  }

  it('knows a person in PostHog by user id, never by email, and marks a bare key as no person', async () => {
    allOn();
    const calls = stubFetch(() => json({}));
    track.accountSignedUp({ id: 'u-1', email: 'ana@example.com' }, 'email');
    track.playbookSaved(KEY, { steps: 3 });
    await settle();
    await drain();
    const batch = calls.filter((c) => c.url.includes('/batch/')).flatMap((c) => JSON.parse(c.init.body).batch);
    const signup = batch.find((e) => e.event === 'account_signed_up');
    assert.deepEqual([signup.distinct_id, signup.properties.$process_person_profile], ['u-1', true]);
    assert.doesNotMatch(JSON.stringify(batch), /ana@example\.com/);
    const keyed = batch.find((e) => e.event === 'playbook_saved');
    assert.equal(keyed.properties.$process_person_profile, false);
    assert.notEqual(keyed.distinct_id, KEY);
  });

  it('returns at once from every seam even when the network never answers', () => {
    allOn();
    stubFetch(() => new Promise(() => {}));
    const started = Date.now();
    track.accountSignedUp({ id: 'u-1' }, 'email');
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
