/**
 * Unit tests for Tx, one optimistic control-store transaction: cached reads,
 * lists merged with the transaction's own changes, the write plan, emitted
 * events, and row locks shared with other in-process transactions.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { SqliteBackend, Tx, columns } from '../../../../../src/modules/control/store.ts';
import { KeyedMutex } from '../../../../../src/modules/control/store/keyed-mutex.ts';
import { scratchDir } from '../../../support/control.ts';

let backend: SqliteBackend;
/** Stores rows as-is, each at version 1. */
const seed = (...rows) =>
  backend.commit({ writes: rows.map(([kind, id, body]) => ({ kind, id, version: 0, body, ...columns(kind, body) })) });

beforeEach(() => {
  backend = new SqliteBackend(join(scratchDir(), 'control.sqlite'), { lock: false });
  seed(
    ['session', 'a', { id: 'a', project: 'p1', state: 'ready', updatedAt: 1 }],
    ['session', 'b', { id: 'b', project: 'p2', state: 'queued' }],
  );
});
afterEach(() => backend.close());

describe('Tx reads', () => {
  it('reads a row once and serves it from the transaction afterwards', async () => {
    const tx = new Tx(backend);
    const load = mock.method(backend, 'load');
    assert.equal((await tx.get('session', 'a')).state, 'ready');
    await tx.get('session', 'a');
    assert.equal(load.mock.callCount(), 1);
  });

  it('reads several rows in one round trip and omits missing ones', async () => {
    const tx = new Tx(backend);
    const load = mock.method(backend, 'load');
    const rows = await tx.getMany('session', ['a', 'nope', 'b']);
    assert.deepEqual(
      rows.map((r) => r.id),
      ['a', 'b'],
    );
    assert.equal(load.mock.callCount(), 1);
    assert.equal(await tx.get('session', 'nope'), null);
  });

  it('lists rows merged with what the transaction put, changed or deleted', async () => {
    const tx = new Tx(backend);
    tx.put('session', 'c', { id: 'c', project: 'p1', state: 'queued' });
    assert.deepEqual((await tx.list('session', { project: 'p1' })).map((r) => r.id).sort(), ['a', 'c']);
    (await tx.get('session', 'a')).state = 'stopped';
    assert.deepEqual(
      (await tx.list('session', { project: 'p1', states: ['ready'] })).map((r) => r.id),
      [],
    );
    await tx.delete('session', 'c');
    assert.deepEqual(
      (await tx.list('session', { project: 'p1' })).map((r) => r.id),
      ['a'],
    );
  });

  it('serves prefetched ids and filters without another round trip', async () => {
    const tx = new Tx(backend);
    await tx.prefetch([
      { kind: 'session', id: 'missing' },
      { kind: 'session', project: 'p2' },
    ]);
    const load = mock.method(backend, 'load');
    assert.equal(await tx.get('session', 'missing'), null);
    assert.deepEqual(
      (await tx.list('session', { project: 'p2' })).map((r) => r.id),
      ['b'],
    );
    assert.equal(load.mock.callCount(), 0);
  });

  it('never lets a later load overwrite a row the transaction already holds', async () => {
    const tx = new Tx(backend);
    (await tx.get('session', 'a')).state = 'mine';
    const [listed] = await tx.list('session', { project: 'p1' });
    assert.equal(listed.state, 'mine');
  });
});

describe('Tx plan', () => {
  it('writes nothing for a transaction that only read', async () => {
    const tx = new Tx(backend);
    await tx.get('session', 'a');
    await tx.get('session', 'missing');
    assert.deepEqual(tx.plan(), { writes: [], events: [] });
  });

  it('writes a changed row under the version it was read at, with its indexed columns', async () => {
    const tx = new Tx(backend);
    (await tx.get('session', 'a')).state = 'stopped';
    const [w] = tx.plan().writes;
    assert.equal(w.version, 1);
    assert.equal(w.state, 'stopped');
    assert.equal(w.project, 'p1');
    assert.ok(w.expiresAt > 0, 'a terminal session gets a retention deadline');
  });

  it('inserts a new row at version 0', () => {
    const tx = new Tx(backend);
    tx.put('meta', 'x', { id: 'x' });
    assert.equal(tx.plan().writes[0].version, 0);
  });

  it('writes a delete as a row with no body', async () => {
    const tx = new Tx(backend);
    await tx.delete('session', 'a');
    assert.deepEqual(tx.plan().writes, [{ kind: 'session', id: 'a', version: 1, body: null }]);
  });

  it('keeps a session’s in-flight count out of its stored body', async () => {
    const tx = new Tx(backend);
    const x = await tx.get('session', 'a');
    assert.equal(x.inFlight, 0);
    x.state = 'stopped';
    assert.equal('inFlight' in tx.plan().writes[0].body, false);
  });

  it('lists what changed per kind, before and after', async () => {
    const tx = new Tx(backend);
    (await tx.get('session', 'a')).state = 'stopped';
    await tx.get('session', 'b');
    assert.deepEqual(
      tx.changes('session').map(({ before, after }) => [before.state, after.state]),
      [['ready', 'stopped']],
    );
  });
});

describe('Tx emit', () => {
  it('records the event with the transaction and bumps its session’s updatedAt', async () => {
    const tx = new Tx(backend);
    const x = await tx.get('session', 'a');
    tx.emit('p1', 'session.ready', 'a', { why: 1 });
    assert.equal(tx.plan().events[0].type, 'session.ready');
    assert.deepEqual(tx.plan().events[0].detail, { why: 1 });
    assert.equal(x.updatedAt, tx.plan().events[0].at);
  });

  it('records an event with no session', () => {
    const tx = new Tx(backend);
    tx.emit('p1', 'project.created');
    assert.equal(tx.plan().events[0].sessionId, null);
  });
});

describe('Tx locks', () => {
  it('creates a missing lock row and always writes a locked one', async () => {
    const tx = new Tx(backend);
    await tx.lock('meta', 'lock');
    await tx.lock('session', 'a');
    assert.deepEqual(
      tx
        .plan()
        .writes.map((w) => `${w.kind} ${w.id} v${w.version}`)
        .sort(),
      ['meta lock v0', 'session a v1'],
    );
  });

  it('makes a second in-process locker wait, then lock on the version the first committed', async () => {
    const mutex = new KeyedMutex();
    const first = new Tx(backend, mutex);
    await first.lock('session', 'a');
    (await first.get('session', 'a')).state = 'stopped';

    const second = new Tx(backend, mutex);
    const cached = await second.get('session', 'a');
    let locked = false;
    const waiting = second.lock('session', 'a').then(() => (locked = true));
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(locked, false, 'waits for the first transaction');

    backend.commit(first.plan());
    first.release();
    await waiting;
    assert.equal(cached.state, 'stopped', 'refreshed in place');
    assert.equal(second.plan().writes[0].version, 2);
    second.release();
  });

  it('takes the in-process lock before reading when asked to acquire', async () => {
    const mutex = new KeyedMutex();
    const tx = new Tx(backend, mutex);
    await tx.acquire('project', 'p1');
    await tx.acquire('project', 'p1');
    let granted = false;
    void mutex.acquire('project p1').then((g) => {
      granted = true;
      g.release();
    });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(granted, false);
    tx.release();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(granted, true);
  });

  it('takes no in-process lock without a mutex', async () => {
    const tx = new Tx(backend);
    await tx.acquire('project', 'p1');
    assert.equal(tx.releases.size, 0);
  });
});
