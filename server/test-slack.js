#!/usr/bin/env node
/**
 * Slack notifications: request signing, message shape, and the delivery worker's
 * Slack branch — including what it does with an install that is gone.
 *
 * No network: globalThis.fetch is replaced, so nothing here reaches Slack.
 */

import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const dir = mkdtempSync(join(tmpdir(), 'oya-slack-'));
process.env.OYA_DATA_DIR = dir;
process.env.OYA_PROFILE_SECRET = 's'.repeat(64);
process.env.OYA_CONSOLE_URL = 'https://console.example.com';
process.env.SLACK_SIGNING_SECRET = 'signing-secret';
const KEY = 'slack-test-key';
process.env.API_KEYS = KEY;
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;

const slack = await import('./src/slack.js');
const { control } = await import('./src/control/service.js');
const { deliver } = await import('./src/control/worker.js');
const keyConfig = await import('./src/key-config.js');

// The shared instance, not a second store: the routes under test reach for it by name.
const service = control();
const store = service.store;

// ── Signature ──

const sign = (body, timestamp, secret = 'signing-secret') =>
  `v0=${createHmac('sha256', secret).update(`v0:${timestamp}:${body}`).digest('hex')}`;
const now = () => String(Math.floor(Date.now() / 1000));

{
  const body = 'payload=%7B%7D', ts = now();
  assert.equal(slack.verifySignature(body, { 'x-slack-request-timestamp': ts, 'x-slack-signature': sign(body, ts) }), true, 'a correct signature verifies');
  assert.equal(slack.verifySignature(body, { 'x-slack-request-timestamp': ts, 'x-slack-signature': sign(body, ts, 'wrong') }), false, 'another secret is rejected');
  assert.equal(slack.verifySignature('payload=%7B%22a%22%3A1%7D', { 'x-slack-request-timestamp': ts, 'x-slack-signature': sign(body, ts) }), false, 'a changed body is rejected');
  const stale = String(Math.floor(Date.now() / 1000) - 3600);
  assert.equal(slack.verifySignature(body, { 'x-slack-request-timestamp': stale, 'x-slack-signature': sign(body, stale) }), false, 'a replayed request is rejected');
  assert.equal(slack.verifySignature(body, {}), false, 'an unsigned request is rejected');
}

// ── Message ──

{
  const attention = { type: 'run.needs_attention', sessionId: 'brw-1', detail: { runId: 'run_9', owner: 'own', reason: 'captcha', message: 'Solve it in the live view.' } };
  const message = slack.blocksFor(attention, 'https://console.example.com/live/brw-1#t=tok');
  const buttons = message.blocks.find(b => b.type === 'actions').elements;
  assert.match(message.text, /Oya needs you/);
  assert.equal(buttons[0].url, 'https://console.example.com/live/brw-1#t=tok', 'the live link is the primary action');
  assert.equal(buttons[1].action_id, 'resume_run');
  assert.deepEqual(JSON.parse(buttons[1].value), { runId: 'run_9', owner: 'own' }, 'the resume button round-trips what respond() needs');

  const failed = slack.blocksFor({ type: 'run.failed', sessionId: 'brw-1', detail: { runId: 'run_9', error: 'Step limit' } }, null);
  const failedActions = failed.blocks.find(b => b.type === 'actions');
  assert.equal(failedActions, undefined, 'a failure with no live browser offers no buttons');
  assert.match(failed.blocks[0].text.text, /Step limit/);
}

// ── Delivery ──

let posted = [];
let reply = { ok: true };
// Only slack.com is stubbed; the test's own requests to its express harness go through.
const realFetch = globalThis.fetch;
globalThis.fetch = async (url, options) => {
  if (!String(url).startsWith('https://slack.com/')) return realFetch(url, options);
  const raw = options?.body;
  posted.push({ url: String(url), body: typeof raw === 'string' ? JSON.parse(raw) : Object.fromEntries(new URLSearchParams(String(raw))) });
  return { json: async () => reply };
};

await keyConfig.saveSlack(KEY, { teamId: 'T1', teamName: 'Acme', botToken: 'xoxb-test', channelId: 'C1', channelName: 'alerts', byo: true });
const session = await service.reserve(KEY, { provider: 'cdp', request: {} });
await service.update(KEY, session.id, { state: 'ready' });
const sink = await service.slackSink(KEY, { channel: 'C1' });
assert.equal(sink.enabled, true);

/** One event through the pipeline; returns the delivery rows it produced. */
async function emitAndDeliver(type, detail) {
  posted = [];
  await service.emit(KEY, type, session.id, detail);
  await deliver(service);
  return store.list('delivery');
}

{
  const deliveries = await emitAndDeliver('run.needs_attention', { runId: 'run_1', owner: 'own', reason: 'mfa', message: 'MFA needs completing.' });
  assert.equal(posted.length, 1, 'the event reached Slack');
  assert.match(posted[0].url, /chat\.postMessage$/);
  assert.equal(posted[0].body.channel, 'C1');
  const link = posted[0].body.blocks.find(b => b.type === 'actions').elements[0].url;
  assert.match(link, /^https:\/\/console\.example\.com\/live\/[^#]+#t=oya_/, 'the message carries a share link for that browser');
  assert.equal(deliveries.every(d => d.state === 'delivered'), true, 'a successful post settles the delivery');
}

{
  // An event type the sink did not subscribe to must not fan out at all.
  posted = [];
  const before = (await store.list('delivery')).length;
  await service.emit(KEY, 'session.ready', session.id, {});
  await deliver(service);
  assert.equal(posted.length, 0, 'unsubscribed events are not posted');
  assert.equal((await store.list('delivery')).length, before, 'and produce no delivery rows');
}

{
  // A deleted channel or revoked token must stop the sink instead of retrying for a day.
  reply = { ok: false, error: 'channel_not_found' };
  await service.emit(KEY, 'run.failed', session.id, { runId: 'run_2', error: 'Broke' });
  await deliver(service);
  const cancelled = (await store.list('delivery')).filter(d => d.state === 'cancelled');
  assert.equal(cancelled.length, 1, 'a dead install cancels the delivery');
  assert.equal((await store.get('webhook', sink.id)).enabled, false, 'and disables the sink');
}

{
  // A transient Slack failure keeps the retry obligation.
  reply = { ok: true };
  await service.slackSink(KEY, { enabled: true });
  reply = { ok: false, error: 'ratelimited' };
  await service.emit(KEY, 'run.failed', session.id, { runId: 'run_3', error: 'Broke again' });
  await deliver(service);
  const pending = (await store.list('delivery')).filter(d => d.state === 'pending' && d.attempts === 1);
  assert.equal(pending.length, 1, 'a transient error stays pending for the backoff');
  assert.equal((await store.get('webhook', sink.id)).enabled, true, 'and leaves the sink alone');
}

// ── OAuth install ──

{
  process.env.SLACK_CLIENT_ID = 'client-id';
  process.env.SLACK_CLIENT_SECRET = 'client-secret';
  const express = (await import('express')).default;
  const app = express();
  app.use(express.json());
  app.use('/api/slack', slack.slackRouter);
  const server = app.listen(0);
  await new Promise(r => server.once('listening', r));
  const base = `http://127.0.0.1:${server.address().port}`;
  const auth = { Authorization: `Bearer ${KEY}` };
  /** The callback as a browser makes it: a top-level GET carrying whatever cookie it holds. */
  const callback = (state, cookie) => fetch(`${base}/api/slack/callback?code=abc&state=${encodeURIComponent(state)}`,
    { redirect: 'manual', headers: cookie ? { cookie } : {} });

  const startInstall = async () => {
    const res = await fetch(`${base}/api/slack/install?json=1`, { headers: auth });
    const { url } = await res.json();
    const setCookie = res.headers.get('set-cookie') || '';
    return { url, state: new URL(url).searchParams.get('state'), cookie: setCookie.split(';')[0] };
  };

  try {
    const { url, state, cookie } = await startInstall();
    assert.match(url, /^https:\/\/slack\.com\/oauth\/v2\/authorize\?/);
    assert.equal(new URL(url).searchParams.get('redirect_uri'), 'https://console.example.com/api/slack/callback');
    assert.ok(state, 'the install URL carries a state');
    assert.equal(cookie, `oya_slack_state=${state}`, 'and the same state is pinned to the browser that asked');
    assert.match(String(await fetch(`${base}/api/slack/install?json=1`, { headers: auth }).then(r => r.headers.get('set-cookie'))), /HttpOnly/i);

    // The attack the state row alone does not stop: an install link built from the
    // attacker's own state, opened in someone else's browser, would file that
    // workspace's token under the attacker's project. No cookie, no install.
    const stolen = await callback(state, undefined);
    assert.equal(stolen.headers.get('location'), '/dashboard?slack=state_mismatch', 'a state replayed in another browser is refused');
    assert.equal(posted.filter(p => p.url.includes('oauth.v2.access')).length, 0, 'and the code is never exchanged');
    const foreign = await callback(state, 'oya_slack_state=a-different-state');
    assert.equal(foreign.headers.get('location'), '/dashboard?slack=state_mismatch', 'a mismatched cookie is refused too');

    reply = { ok: true, team: { id: 'T2', name: 'Installed' }, access_token: 'xoxb-installed' };
    const landing = await callback(state, cookie);
    assert.equal(landing.headers.get('location'), '/dashboard?slack=connected', 'an install that kept its channel goes straight back');
    const install = keyConfig.getSlack(KEY);
    assert.equal(install.botToken, 'xoxb-installed', 'the OAuth token replaces the pasted one');
    assert.equal(install.byo, false);
    assert.equal(install.channelId, 'C1', 'and the channel already chosen survives a reinstall');

    // A state redeems once, so a replayed callback cannot install a second time.
    const replayed = await callback(state, cookie);
    assert.equal(replayed.headers.get('location'), '/dashboard?slack=expired', 'a reused state is refused');
  } finally { server.close(); }
}

console.log('Slack signing, message shape, delivery and install checks passed');
store.close?.();
rmSync(dir, { recursive: true, force: true });
