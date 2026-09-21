/**
 * Unit tests for the session helpers: the public view, and stopping a session.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { publicSession, stopSession } from '../../../../../src/modules/control/service/sessions.ts';

/** A transaction stand-in that records emitted events. */
const recorder = () => {
  const events = [];
  return { events, emit: (...args) => events.push(args) };
};

describe('publicSession', () => {
  it('drops what only the server may see', () => {
    const x = publicSession({
      id: 's',
      state: 'ready',
      cleanup: {},
      response: {},
      requestHash: 'h',
      queuedRequest: 'q',
      egressHash: 'e',
      enrollmentHash: 'n',
    });
    assert.deepEqual(x, { id: 's', state: 'ready' });
  });
});

describe('stopSession', () => {
  it('stops a queued session outright', () => {
    const tx = recorder(),
      x = { id: 's', project: 'p', state: 'queued' };
    stopSession(tx, x);
    assert.equal(x.state, 'stopped');
    assert.deepEqual(tx.events, [['p', 'session.stopped', 's', { reason: 'cancelled' }]]);
  });

  it('sends anything else to cleanup with the given reason', () => {
    const tx = recorder(),
      x = { id: 's', project: 'p', state: 'ready' };
    stopSession(tx, x, { reason: 'budget' });
    assert.equal(x.state, 'cleanup_pending');
    assert.deepEqual(tx.events[0][3], { reason: 'budget' });
  });

  it('with force, reconciles only a session with no descriptor and no live creation', () => {
    const plain = { id: 'a', project: 'p', state: 'ready' };
    const described = { id: 'b', project: 'p', state: 'ready', cleanup: {} };
    const creating = { id: 'c', project: 'p', state: 'provisioning', provisioningActive: true, leaseUntil: Infinity };
    for (const x of [plain, described, creating]) stopSession(recorder(), x, { force: true });
    assert.deepEqual([plain.state, described.state, creating.state], ['stopped', 'cleanup_pending', 'cleanup_pending']);
  });
});
