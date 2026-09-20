/**
 * Unit tests for the row-to-column mapping: which body field each kind indexes,
 * when the pruner may delete a row, and how indexed filters match.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { columns, matches } from '../../../../../src/modules/control/store/columns.ts';
import {
  DAY_MS,
  DELIVERY_TTL_MS,
  IDEMPOTENCY_TTL_MS,
  INSTANCE_TTL_MS,
  TERMINAL_SESSION_TTL_MS,
} from '../../../../../src/modules/control/store/constants.ts';

describe('columns', () => {
  it('indexes a session by its project and state', () => {
    assert.deepEqual(columns('session', { project: 'p1', state: 'ready' }), {
      project: 'p1',
      state: 'ready',
      expiresAt: null,
    });
  });

  it('indexes each kind by the field it is looked up by', () => {
    assert.equal(columns('credential', { role: 'browser' }).state, 'browser');
    assert.equal(columns('hold', { resource: '@shared:x' }).state, '@shared:x');
    assert.equal(columns('attachment', { instance: 'i-1' }).state, 'i-1');
    assert.equal(columns('membership', { userId: 'u-1' }).state, 'u-1');
    assert.equal(columns('project', { ownerUser: 'u-2' }).state, 'u-2');
    assert.equal(columns('recording', { owner: 'o-1' }).state, 'o-1');
  });

  it('leaves missing project and state null', () => {
    assert.deepEqual(columns('meta', { id: 'draining' }), { project: null, state: null, expiresAt: null });
  });

  it('keeps a live session forever and a terminal one a week after its last update', () => {
    assert.equal(columns('session', { state: 'ready', updatedAt: 5 }).expiresAt, null);
    assert.equal(columns('session', { state: 'stopped', updatedAt: 5 }).expiresAt, 5 + TERMINAL_SESSION_TTL_MS);
    assert.equal(columns('session', { state: 'failed', createdAt: 7 }).expiresAt, 7 + TERMINAL_SESSION_TTL_MS);
    assert.equal(TERMINAL_SESSION_TTL_MS, 7 * DAY_MS);
  });

  it('expires tickets, invites, holds and Slack state at their own expiresAt', () => {
    for (const kind of ['ticket', 'invite', 'hold', 'slack_state'])
      assert.equal(columns(kind, { expiresAt: 42 }).expiresAt, 42, kind);
  });

  it('expires attachments at their lease, instances a day after it, idempotency records a week after creation', () => {
    assert.equal(columns('attachment', { leaseUntil: 10 }).expiresAt, 10);
    assert.equal(columns('instance', { leaseUntil: 10 }).expiresAt, 10 + INSTANCE_TTL_MS);
    assert.equal(columns('idempotency', { createdAt: 10 }).expiresAt, 10 + IDEMPOTENCY_TTL_MS);
  });

  it('keeps a pending delivery and expires a settled one thirty days after its event', () => {
    assert.equal(columns('delivery', { state: 'pending', at: 1 }).expiresAt, null);
    assert.equal(columns('delivery', { state: 'delivered', at: 1 }).expiresAt, 1 + DELIVERY_TTL_MS);
  });

  it('never expires kinds with no retention rule', () => {
    assert.equal(columns('project', { expiresAt: 1 }).expiresAt, null);
    assert.equal(columns('webhook', { expiresAt: 1 }).expiresAt, null);
  });
});

describe('matches', () => {
  const body = { project: 'p1', state: 'ready' };

  it('matches everything with an empty filter', () => {
    assert.equal(matches('session', body, {}), true);
  });

  it('matches on project', () => {
    assert.equal(matches('session', body, { project: 'p1' }), true);
    assert.equal(matches('session', body, { project: 'p2' }), false);
  });

  it('matches on the indexed state', () => {
    assert.equal(matches('session', body, { states: ['queued', 'ready'] }), true);
    assert.equal(matches('session', body, { states: ['queued'] }), false);
  });
});
