/**
 * Unit tests for the control plane's vocabulary: project ids derived from keys,
 * which states hold a slot, coded faults, and moving a session between states.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  attachOnly,
  capacityReached,
  fault,
  hash,
  holdsSlot,
  live,
  moveTo,
  projectDeleted,
  projectId,
  sessionNotFound,
  terminal,
} from '../../../../../src/modules/control/service/model.ts';

describe('projectId', () => {
  it('derives a stable prj_ id from the key, without any lookup', () => {
    assert.match(projectId('key-a'), /^prj_[0-9a-f]{24}$/);
    assert.equal(projectId('key-a'), projectId('key-a'));
    assert.equal(projectId('key-a'), `prj_${hash('key-a').slice(0, 24)}`);
  });

  it('gives different keys different projects', () => {
    assert.notEqual(projectId('key-a'), projectId('key-b'));
  });
});

describe('session states', () => {
  it('treats stopped and failed as terminal and everything else as live', () => {
    assert.deepEqual([...terminal], ['stopped', 'failed']);
    for (const state of live) assert.equal(terminal.has(state), false);
  });

  it('holds a slot while a resource may exist, not while queued, disconnected or over', () => {
    for (const state of ['provisioning', 'ready', 'stopping', 'cleanup_pending', 'unknown_outcome'])
      assert.equal(holdsSlot({ state }), true, state);
    for (const state of ['queued', 'disconnected', 'stopped', 'failed'])
      assert.equal(holdsSlot({ state }), false, state);
  });

  it('knows which providers have no resource of ours to delete', () => {
    assert.deepEqual([...attachOnly].sort(), ['cdp', 'gateway', 'oya-desktop']);
  });
});

describe('faults', () => {
  it('carries a code and defaults to 409', () => {
    const e = fault('x', 'message');
    assert.equal(e.status, 409);
    assert.equal(e.code, 'x');
    assert.equal(e.message, 'message');
  });

  it('names the common refusals', () => {
    assert.deepEqual([sessionNotFound().status, sessionNotFound().code], [404, 'not_found']);
    assert.deepEqual([projectDeleted().status, projectDeleted().code], [410, 'project_deleted']);
    assert.deepEqual([capacityReached().status, capacityReached().code], [429, 'quota_exceeded']);
  });
});

describe('moveTo', () => {
  it('moves the session and records the change as an event', () => {
    const events = [],
      tx = { emit: (...args) => events.push(args) };
    const x = { id: 's', project: 'p', state: 'provisioning' };
    moveTo(tx, x, 'ready');
    assert.equal(x.state, 'ready');
    assert.deepEqual(events, [['p', 'session.ready', 's']]);
  });

  it('does nothing when the session is already there', () => {
    const events = [];
    moveTo({ emit: (...a) => events.push(a) }, { state: 'ready' }, 'ready');
    assert.deepEqual(events, []);
  });
});
