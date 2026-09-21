/**
 * Unit tests for SqliteBackend: loading rows by id, project and state,
 * compare-and-swap commits, the event log and the webhook deliveries it queues,
 * pruning, the session command gate, and the single-writer lock.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { SqliteBackend, columns } from '../../../../../src/modules/control/store.ts';
import { scratchDir } from '../../../support/control.ts';

/** A version-checked write of `body` (null deletes). */
const write = (kind, id, body, version = 0) => ({ kind, id, version, body, ...(body ? columns(kind, body) : {}) });

/** A ready session on instance `owner`, under agent control. */
const session = (id, project = 'p1', extra = {}) => ({
  id,
  project,
  state: 'ready',
  fence: 1,
  instance: 'i-1',
  control: { mode: 'agent' },
  ...extra,
});

let backend: SqliteBackend;
beforeEach(() => {
  backend = new SqliteBackend(join(scratchDir(), 'control.sqlite'), { lock: false });
});
afterEach(() => backend.close());

describe('SqliteBackend rows', () => {
  it('inserts a row at version 1 and loads it by id', () => {
    assert.deepEqual(backend.commit({ writes: [write('meta', 'm', { id: 'm', value: 1 })] }), { ok: true, events: [] });
    const [[row]] = backend.load([{ kind: 'meta', id: 'm' }]);
    assert.deepEqual(row, { id: 'm', version: 1, body: { id: 'm', value: 1 } });
  });

  it('loads by project and by indexed state', () => {
    backend.commit({
      writes: [
        write('session', 'a', session('a', 'p1')),
        write('session', 'b', session('b', 'p2')),
        write('session', 'c', session('c', 'p1', { state: 'queued' })),
      ],
    });
    const [byProject, byState, both] = backend.load([
      { kind: 'session', project: 'p1' },
      { kind: 'session', states: ['queued'] },
      { kind: 'session', project: 'p2', states: ['queued'] },
    ]);
    assert.deepEqual(byProject.map((r) => r.id).sort(), ['a', 'c']);
    assert.deepEqual(
      byState.map((r) => r.id),
      ['c'],
    );
    assert.deepEqual(both, []);
  });

  it('answers an empty state list with no rows', () => {
    backend.commit({ writes: [write('session', 'a', session('a'))] });
    assert.deepEqual(backend.load([{ kind: 'session', states: [] }]), [[]]);
  });

  it('bumps the version on each replace', () => {
    backend.commit({ writes: [write('meta', 'm', { id: 'm' })] });
    assert.equal(backend.commit({ writes: [write('meta', 'm', { id: 'm', v: 2 }, 1)] }).ok, true);
    assert.equal(backend.load([{ kind: 'meta', id: 'm' }])[0][0].version, 2);
  });

  it('refuses a write whose version moved since it was read, applying none of the transaction', () => {
    backend.commit({ writes: [write('meta', 'm', { id: 'm', v: 1 })] });
    const result = backend.commit({
      writes: [write('meta', 'new', { id: 'new' }), write('meta', 'm', { id: 'm', v: 2 }, 0)],
    });
    assert.deepEqual(result, { ok: false });
    assert.deepEqual(backend.load([{ kind: 'meta', id: 'new' }]), [[]]);
    assert.equal(backend.load([{ kind: 'meta', id: 'm' }])[0][0].body.v, 1);
  });

  it('deletes a row written with no body', () => {
    backend.commit({ writes: [write('meta', 'm', { id: 'm' })] });
    backend.commit({ writes: [write('meta', 'm', null, 1)] });
    assert.deepEqual(backend.load([{ kind: 'meta', id: 'm' }]), [[]]);
  });
});

describe('SqliteBackend events', () => {
  it('appends events with increasing sequence numbers', () => {
    const { events } = backend.commit({
      events: [
        { project: 'p1', type: 'a', at: 1 },
        { project: 'p1', type: 'b', at: 2, sessionId: 's', detail: { x: 1 } },
      ],
    });
    assert.equal(events[1], events[0] + 1);
    assert.deepEqual(backend.events({ project: 'p1' })[1], {
      id: events[1],
      project: 'p1',
      type: 'b',
      sessionId: 's',
      at: 2,
      detail: { x: 1 },
    });
  });

  it('reads only the asked project’s events, after the cursor', () => {
    const { events } = backend.commit({
      events: [
        { project: 'p1', type: 'a', at: 1 },
        { project: 'p2', type: 'other', at: 1 },
        { project: 'p1', type: 'b', at: 2 },
      ],
    });
    assert.deepEqual(
      backend.events({ project: 'p1' }).map((e) => e.type),
      ['a', 'b'],
    );
    assert.deepEqual(
      backend.events({ project: 'p1', after: events[0] }).map((e) => e.type),
      ['b'],
    );
  });

  it('reads the latest events oldest first', () => {
    backend.commit({ events: ['a', 'b', 'c'].map((type, at) => ({ project: 'p1', type, at })) });
    assert.deepEqual(
      backend.events({ project: 'p1', latest: true, limit: 2 }).map((e) => e.type),
      ['b', 'c'],
    );
  });

  it('reads specific events by sequence, and nothing for an empty list', () => {
    const { events } = backend.commit({ events: ['a', 'b'].map((type) => ({ project: 'p1', type, at: 1 })) });
    assert.deepEqual(
      backend.events({ seqs: [events[1]] }).map((e) => e.type),
      ['b'],
    );
    assert.deepEqual(backend.events({ seqs: [] }), []);
  });
});

describe('SqliteBackend webhook deliveries', () => {
  /** Stores a webhook for a project. */
  const hook = (id, project, types = [], enabled = true) =>
    backend.commit({ writes: [write('webhook', id, { id, project, types, enabled })] });

  it('queues a pending delivery for each enabled hook in the event’s project that wants its type', () => {
    hook('all', 'p1');
    hook('ready-only', 'p1', ['session.ready']);
    const { events } = backend.commit({ events: [{ project: 'p1', type: 'session.ready', at: 5 }] });
    const deliveries = backend.load([{ kind: 'delivery', states: ['pending'] }])[0].map((r) => r.body);
    assert.deepEqual(deliveries.map((d) => d.hook).sort(), ['all', 'ready-only']);
    assert.deepEqual(deliveries[0], {
      id: `${deliveries[0].hook}:${events[0]}`,
      hook: deliveries[0].hook,
      project: 'p1',
      eventSeq: events[0],
      at: 5,
      attempts: 0,
      nextAt: 5,
      state: 'pending',
    });
  });

  it('queues nothing for a disabled hook or one that does not want the type', () => {
    hook('off', 'p1', [], false);
    hook('failed-only', 'p1', ['session.failed']);
    backend.commit({ events: [{ project: 'p1', type: 'session.ready', at: 5 }] });
    assert.deepEqual(backend.load([{ kind: 'delivery' }]), [[]]);
  });

  it('never delivers one project’s events to another project’s hook', () => {
    hook('theirs', 'p2');
    backend.commit({ events: [{ project: 'p1', type: 'session.ready', at: 5 }] });
    assert.deepEqual(backend.load([{ kind: 'delivery' }]), [[]]);
  });
});

describe('SqliteBackend prune', () => {
  it('deletes rows past their expiry and keeps the rest', () => {
    backend.commit({
      writes: [write('ticket', 'old', { expiresAt: 10 }), write('ticket', 'new', { expiresAt: 1000 })],
    });
    backend.prune(100);
    assert.deepEqual(
      backend.load([{ kind: 'ticket' }])[0].map((r) => r.id),
      ['new'],
    );
  });

  it('removes the gate of a pruned session with it', () => {
    backend.commit({ writes: [write('session', 's', session('s', 'p1', { state: 'stopped', updatedAt: 1 }))] });
    backend.prune(Number.MAX_SAFE_INTEGER);
    assert.equal(backend.beginCommand('s', null, 'i-1'), null);
  });

  it('deletes each project’s events older than its own cutoff', () => {
    backend.commit({
      events: [
        { project: 'p1', type: 'old', at: 1 },
        { project: 'p1', type: 'new', at: 100 },
        { project: 'p2', type: 'kept', at: 1 },
      ],
    });
    backend.prune(0, { p1: 50 });
    assert.deepEqual(
      backend.events({ project: 'p1' }).map((e) => e.type),
      ['new'],
    );
    assert.equal(backend.events({ project: 'p2' }).length, 1);
  });
});

describe('SqliteBackend command gate', () => {
  /** Stores or replaces session `s`. */
  const put = (body, version) => backend.commit({ writes: [write('session', 's', body, version)] });

  it('admits an agent command, returning the fence and counting it in flight', () => {
    put(session('s', 'p1', { fence: 3 }), 0);
    assert.equal(backend.beginCommand('s', null, 'i-1'), 3);
    assert.equal(backend.load([{ kind: 'session', id: 's' }])[0][0].body.inFlight, 1);
    backend.finishCommand('s', 3);
    assert.equal(backend.load([{ kind: 'session', id: 's' }])[0][0].body.inFlight, 0);
  });

  it('answers null for a session it has no gate for', () => {
    assert.equal(backend.beginCommand('nobody', null, 'i-1'), null);
  });

  it('refuses a command from another replica, for a session that is not ready, or while paused', () => {
    put(session('s'), 0);
    assert.throws(() => backend.beginCommand('s', null, 'i-2'), { code: 'control_paused', status: 409 });
    put(session('s', 'p1', { state: 'provisioning' }), 1);
    assert.throws(() => backend.beginCommand('s', null, 'i-1'), { code: 'control_paused' });
    put(session('s', 'p1', { control: { mode: 'paused' } }), 2);
    assert.throws(() => backend.beginCommand('s', null, 'i-1'), { code: 'control_paused' });
  });

  it('admits only the live human holder during a takeover', () => {
    put(session('s', 'p1', { control: { mode: 'human', holder: 'h', expiresAt: Date.now() + 60_000 } }), 0);
    assert.throws(() => backend.beginCommand('s', null, 'i-1'), { code: 'control_paused' });
    assert.throws(() => backend.beginCommand('s', 'other', 'i-1'), { code: 'control_paused' });
    assert.equal(backend.beginCommand('s', 'h', 'i-1'), 1);
  });

  it('refuses a lapsed human hold', () => {
    put(session('s', 'p1', { control: { mode: 'human', holder: 'h', expiresAt: Date.now() - 1 } }), 0);
    assert.throws(() => backend.beginCommand('s', 'h', 'i-1'), { code: 'control_paused' });
  });

  it('refuses to hand a session to a human while agent commands are in flight, rolling the commit back', () => {
    put(session('s'), 0);
    backend.beginCommand('s', null, 'i-1');
    const human = session('s', 'p1', { control: { mode: 'human', holder: 'h', expiresAt: Date.now() + 60_000 } });
    assert.throws(() => put(human, 1), { code: 'commands_pending', status: 409 });
    assert.equal(backend.load([{ kind: 'session', id: 's' }])[0][0].body.control.mode, 'agent');
  });

  it('zeroes the in-flight count when the session is re-placed under a new fence', () => {
    put(session('s'), 0);
    backend.beginCommand('s', null, 'i-1');
    put(session('s', 'p1', { fence: 2 }), 1);
    assert.equal(backend.load([{ kind: 'session', id: 's' }])[0][0].body.inFlight, 0);
  });

  it('ignores a finish under a stale fence, and one with no fence', () => {
    put(session('s'), 0);
    backend.beginCommand('s', null, 'i-1');
    backend.finishCommand('s', 99);
    backend.finishCommand('s', null);
    assert.equal(backend.load([{ kind: 'session', id: 's' }])[0][0].body.inFlight, 1);
  });

  it('drops the gate when its session row is deleted', () => {
    put(session('s'), 0);
    put(null, 1);
    assert.equal(backend.beginCommand('s', null, 'i-1'), null);
  });
});

describe('SqliteBackend lock', () => {
  it('turns away a second server on the same database until the first closes', () => {
    const path = join(scratchDir(), 'control.sqlite');
    const first = new SqliteBackend(path);
    assert.throws(() => new SqliteBackend(path), /already in use/);
    first.close();
    new SqliteBackend(path).close();
  });
});
