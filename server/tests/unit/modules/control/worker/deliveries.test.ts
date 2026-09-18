/**
 * Unit tests for webhook delivery: due deliveries claimed under a lease and
 * sent signed, outcomes recorded with backoff, and deliveries to a disabled
 * hook or older than a day closed instead of sent.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { projectId } from '../../../../../src/modules/control/service.ts';
import { deliver } from '../../../../../src/modules/control/worker/deliveries.ts';
import { patchRow, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  NOW = 1_700_000_000_000;
let service;
beforeEach(async () => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
  await service.webhook(A, { url: 'https://h.example/a', types: ['session.ready'] });
});
afterEach(() => mock.timers.reset());

/** A sender that answers `ok` and records what it was given. */
function sender(ok = true) {
  const calls = [];
  const send = async (url, body, headers) => (calls.push({ url, body: JSON.parse(body), headers }), ok);
  return Object.assign(send, { calls });
}
/** The project's deliveries. */
const deliveries = () => service.store.list('delivery', { project: projectId(A) });

describe('deliver', () => {
  it('sends a due delivery signed, and marks it delivered', async () => {
    await service.emit(A, 'session.ready', 's1');
    const send = sender();
    assert.equal(await deliver(service, send), 1);
    assert.equal(send.calls[0].url, 'https://h.example/a');
    assert.equal(send.calls[0].body.type, 'session.ready');
    assert.match(send.calls[0].headers['Oya-Signature'], /^t=\d+,v1=[0-9a-f]{64}$/);
    const [d] = await deliveries();
    assert.deepEqual([d.state, d.attempts, d.leaseUntil], ['delivered', 1, null]);
  });

  it('keeps a failed delivery pending and backs off before the next try', async () => {
    await service.emit(A, 'session.ready');
    await deliver(service, sender(false));
    const [d] = await deliveries();
    assert.deepEqual([d.state, d.nextAt], ['pending', NOW + 2000]);
    const send = sender();
    await deliver(service, send);
    assert.equal(send.calls.length, 0, 'not due yet');
  });

  it('retries a delivery whose sender throws', async () => {
    await service.emit(A, 'session.ready');
    await deliver(service, async () => {
      throw new Error('ECONNRESET');
    });
    assert.equal((await deliveries())[0].state, 'pending');
  });

  it('cancels deliveries to a disabled hook without sending them', async () => {
    await service.emit(A, 'session.ready');
    await patchRow(service, 'webhook', `hook:${projectId(A)}`, { enabled: false });
    const send = sender();
    await deliver(service, send);
    assert.equal(send.calls.length, 0);
    assert.equal((await deliveries())[0].state, 'cancelled');
  });

  it('fails a delivery that has been retrying for more than a day', async () => {
    await service.emit(A, 'session.ready');
    mock.timers.tick(86_400_001);
    await deliver(service, sender());
    assert.equal((await deliveries())[0].state, 'failed');
  });

  it('counts a day from a replay rather than the original event', async () => {
    await service.emit(A, 'session.ready');
    const [d] = await deliveries();
    mock.timers.tick(86_400_001);
    await patchRow(service, 'delivery', d.id, { replayAt: Date.now() });
    const send = sender();
    await deliver(service, send);
    assert.equal(send.calls.length, 1);
  });

  it('leaves a delivery another claim holds alone', async () => {
    await service.emit(A, 'session.ready');
    const [d] = await deliveries();
    await patchRow(service, 'delivery', d.id, { leaseUntil: NOW + 1 });
    const send = sender();
    await deliver(service, send);
    assert.equal(send.calls.length, 0);
  });

  it('retries a delivery whose event is past retention', async () => {
    await service.emit(A, 'session.ready');
    await service.store.prune(NOW, { [projectId(A)]: NOW + 1 });
    const send = sender();
    await deliver(service, send);
    assert.equal(send.calls.length, 0);
    assert.equal((await deliveries())[0].state, 'pending');
  });

  it('sends at most eight per pass', async () => {
    for (let i = 0; i < 10; i++) await service.emit(A, 'session.ready');
    const send = sender();
    assert.equal(await deliver(service, send), 10);
    assert.equal(send.calls.length, 8);
  });
});
