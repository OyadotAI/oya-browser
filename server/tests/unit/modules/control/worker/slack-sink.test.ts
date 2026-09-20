/**
 * Unit tests for Slack as a delivery sink: one event as a channel message with
 * a live link for browser events, retry when the install cannot be read, and
 * a dead install cancelling its deliveries and disabling the sink.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-slack-');
const { projectId } = await import('../../../../../src/modules/control/service.ts');
const { postSlack } = await import('../../../../../src/modules/control/worker/slack-sink.ts');
const { deliver } = await import('../../../../../src/modules/control/worker/deliveries.ts');
const keyConfig = await import('../../../../../src/modules/config/service.ts');
const { json, stubFetch } = await import('../../../support/http.ts');
const { readySession, scratchService } = await import('../../../support/control.ts');

const A = 'key-a';
let service;
beforeEach(async () => {
  service = scratchService();
  await service.project(A);
});
afterEach(async () => {
  mock.restoreAll();
  await keyConfig.clearSlack(A);
});

/** The project's Slack hook row. */
const hook = (channel = null) => ({ id: `slack:${projectId(A)}`, project: projectId(A), kind: 'slack', channel });
/** An event of the project. */
const event = (sessionId = null) => ({
  id: 1,
  project: projectId(A),
  type: 'run.failed',
  sessionId,
  at: 1,
  detail: {},
});

describe('postSlack', () => {
  it('retries while the project has no Slack install', async () => {
    const calls = stubFetch(() => json({ ok: true }));
    assert.deepEqual(await postSlack(service, hook('C1'), event()), { ok: false, dead: false });
    assert.equal(calls.length, 0);
  });

  it('retries when the project’s key cannot be found', async () => {
    assert.deepEqual(await postSlack(service, { ...hook(), project: 'prj_gone' }, event()), {
      ok: false,
      dead: false,
    });
  });

  it('posts to the install’s channel with its bot token', async () => {
    await keyConfig.saveSlack(A, { botToken: 'xoxb-1', channelId: 'C9' });
    const calls = stubFetch(() => json({ ok: true }));
    assert.deepEqual(await postSlack(service, hook(), event()), { ok: true, dead: false });
    assert.match(calls[0].url, /chat\.postMessage$/);
    assert.equal(calls[0].init.headers.Authorization, 'Bearer xoxb-1');
    assert.equal(JSON.parse(calls[0].init.body).channel, 'C9');
  });

  it('prefers the sink’s own channel', async () => {
    await keyConfig.saveSlack(A, { botToken: 'xoxb-1', channelId: 'C9' });
    const calls = stubFetch(() => json({ ok: true }));
    await postSlack(service, hook('C1'), event());
    assert.equal(JSON.parse(calls[0].init.body).channel, 'C1');
  });

  it('adds a live link, a control share of that browser, for a browser event', async () => {
    await keyConfig.saveSlack(A, { botToken: 'xoxb-1', channelId: 'C9' });
    await readySession(service, A, 's1');
    const calls = stubFetch(() => json({ ok: true }));
    await postSlack(service, hook(), event('s1'));
    assert.match(calls[0].init.body, /\/live\/s1#t=oya_/);
    assert.ok((await service.read(A)).credentials.some((c) => c.label === 'Shared browser (control)'));
  });

  it('still posts, without a link, for a browser that has ended', async () => {
    await keyConfig.saveSlack(A, { botToken: 'xoxb-1', channelId: 'C9' });
    const calls = stubFetch(() => json({ ok: true }));
    assert.equal((await postSlack(service, hook(), event('gone'))).ok, true);
    assert.doesNotMatch(calls[0].init.body, /\/live\//);
  });

  it('reports a revoked token or deleted channel as dead, anything else as retryable', async () => {
    await keyConfig.saveSlack(A, { botToken: 'xoxb-1', channelId: 'C9' });
    stubFetch(() => json({ ok: false, error: 'token_revoked' }));
    assert.deepEqual(await postSlack(service, hook(), event()), { ok: false, dead: true });
    mock.restoreAll();
    stubFetch(() => json({ ok: false, error: 'ratelimited' }));
    assert.deepEqual(await postSlack(service, hook(), event()), { ok: false, dead: false });
  });
});

describe('deliver to a dead Slack install', () => {
  it('cancels the delivery and disables the sink', async () => {
    await keyConfig.saveSlack(A, { botToken: 'xoxb-1', channelId: 'C9' });
    const sink = await service.slackSink(A, { types: ['run.failed'] });
    await service.emit(A, 'run.failed');
    stubFetch(() => json({ ok: false, error: 'channel_not_found' }));
    await deliver(service);
    const [d] = await service.store.list('delivery', { project: projectId(A) });
    assert.equal(d.state, 'cancelled');
    assert.equal((await service.store.get('webhook', sink.id)).enabled, false);
  });
});
