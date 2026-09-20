/**
 * Unit tests for where a project's events go: its one customer webhook (secret
 * minted once, sealed, rolled on request), the settings view of it, the Slack
 * sink, and events emitted from outside the session state machine.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { projectId, SLACK_EVENTS, WEBHOOK_EVENTS } from '../../../../../src/modules/control/service.ts';
import { openText } from '../../../../../src/platform/secrets.ts';
import { scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b';
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: 1_700_000_000_000 });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

/** The stored hook row. */
const storedHook = (key) => service.store.get('webhook', `hook:${projectId(key)}`);

describe('webhook', () => {
  it('mints a secret on first save, returns it once and stores it sealed', async () => {
    const hook = await service.webhook(A, { url: 'https://h.example/a' });
    assert.equal(hook.id, `hook:${projectId(A)}`);
    assert.ok(hook.secret);
    const row = await storedHook(A);
    assert.notEqual(row.secret, hook.secret);
    assert.equal(openText(`webhook:${hook.id}`, row.secret), hook.secret);
    assert.equal((await service.events(A)).at(-1).type, 'webhook.created');
  });

  it('edits the one hook in place, keeping its secret and not returning it', async () => {
    const first = await service.webhook(A, { url: 'https://h.example/a' });
    const second = await service.webhook(A, { url: 'https://h.example/b', types: ['session.ready'] });
    assert.equal(second.id, first.id);
    assert.equal('secret' in second, false);
    assert.equal(openText(`webhook:${first.id}`, (await storedHook(A)).secret), first.secret);
    assert.equal((await service.events(A)).at(-1).type, 'webhook.updated');
  });

  it('mints a new secret when asked to roll it', async () => {
    const first = await service.webhook(A, { url: 'https://h.example/a' });
    const rolled = await service.webhook(A, { url: 'https://h.example/a', roll: true });
    assert.ok(rolled.secret);
    assert.notEqual(rolled.secret, first.secret);
  });

  it('refuses event types that are not a list of strings', async () => {
    await assert.rejects(service.webhook(A, { url: 'u', types: 'session.ready' }), {
      status: 400,
      code: 'invalid_events',
    });
    await assert.rejects(service.webhook(A, { url: 'u', types: [1] }), { code: 'invalid_events' });
  });
});

describe('webhookConfig', () => {
  it('answers a null hook and the subscribable events before one is set', async () => {
    assert.deepEqual(await service.webhookConfig(A), { hook: null, events: WEBHOOK_EVENTS, deliveries: [] });
  });

  it('shows the hook without its secret, and its deliveries newest first with their event types', async () => {
    await service.webhook(A, { url: 'https://h.example/a', types: ['session.ready', 'session.failed'] });
    await service.emit(A, 'session.ready');
    mock.timers.tick(1);
    await service.emit(A, 'session.failed');
    const config = await service.webhookConfig(A);
    assert.deepEqual(config.hook, {
      id: `hook:${projectId(A)}`,
      url: 'https://h.example/a',
      types: ['session.ready', 'session.failed'],
      enabled: true,
    });
    assert.deepEqual(
      config.deliveries.map((d) => [d.type, d.state]),
      [
        ['session.failed', 'pending'],
        ['session.ready', 'pending'],
      ],
    );
  });

  it('never lists another project’s deliveries', async () => {
    await service.webhook(A, { url: 'https://h.example/a' });
    await service.webhook(B, { url: 'https://h.example/b' });
    await service.emit(B, 'session.ready');
    const { deliveries } = await service.webhookConfig(A);
    assert.ok(deliveries.every((d) => d.type !== 'session.ready'));
  });
});

describe('slackSink', () => {
  it('creates one sink per project with the default alert types', async () => {
    const sink = await service.slackSink(A);
    assert.deepEqual(sink, { id: `slack:${projectId(A)}`, channel: null, types: SLACK_EVENTS, enabled: true });
    const row = await service.store.get('webhook', sink.id);
    assert.deepEqual([row.kind, row.url], ['slack', null]);
  });

  it('changes only what the caller passed', async () => {
    await service.slackSink(A, { channel: 'C1' });
    const sink = await service.slackSink(A, { enabled: false });
    assert.deepEqual([sink.channel, sink.enabled], ['C1', false]);
    assert.deepEqual((await service.events(A)).at(-1).detail, { id: sink.id, kind: 'slack' });
  });
});

describe('emit', () => {
  it('records an outside event on the key’s project', async () => {
    await service.emit(A, 'run.failed', 's1', { why: 'x' });
    const [e] = await service.events(A);
    assert.deepEqual([e.type, e.sessionId, e.detail], ['run.failed', 's1', { why: 'x' }]);
    assert.deepEqual(await service.events(B), []);
  });
});
