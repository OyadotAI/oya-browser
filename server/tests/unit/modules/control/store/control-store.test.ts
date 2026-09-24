/**
 * Unit tests for ControlStore: transactions that commit, roll back, queue one at
 * a time on SQLite, retry on a remote version conflict and give up with
 * storage_contention; the beforeCommit hook; reads and delegated calls.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ControlStore } from '../../../../../src/modules/control/store.ts';
import { MAX_TX_ATTEMPTS } from '../../../../../src/modules/control/store/constants.ts';
import { scratchStore } from '../../../support/control.ts';

afterEach(() => mock.restoreAll());

describe('ControlStore over SQLite', () => {
  it('commits a transaction and reads it back by id and by filter', async () => {
    const store = scratchStore();
    await store.transact(async (tx) => {
      tx.put('session', 's', { id: 's', project: 'p1', state: 'ready' });
    });
    assert.equal((await store.get('session', 's')).state, 'ready');
    assert.equal(await store.get('session', 'missing'), null);
    assert.deepEqual(
      (await store.list('session', { project: 'p1' })).map((x) => x.id),
      ['s'],
    );
    assert.equal((await store.load([{ kind: 'session', id: 's' }]))[0][0].version, 1);
  });

  it('hands back a copy of the result, not the transaction’s row', async () => {
    const store = scratchStore();
    const row = await store.transact(async (tx) => tx.put('meta', 'm', { id: 'm', n: 1 }));
    row.n = 99;
    assert.equal((await store.get('meta', 'm')).n, 1);
  });

  it('commits nothing from a transaction that throws, and keeps serving later ones', async () => {
    const store = scratchStore();
    const failed = store.transact(async (tx) => {
      tx.put('meta', 'm', { id: 'm' });
      throw new Error('boom');
    });
    await assert.rejects(failed, /boom/);
    await store.transact(async (tx) => void tx.put('meta', 'n', { id: 'n' }));
    assert.equal(await store.get('meta', 'm'), null);
    assert.ok(await store.get('meta', 'n'));
  });

  it('runs concurrent transactions one at a time so none of their updates are lost', async () => {
    const store = scratchStore();
    const bump = () =>
      store.transact(async (tx) => {
        const counter = (await tx.get('meta', 'c')) || tx.put('meta', 'c', { id: 'c', n: 0 });
        counter.n += 1;
      });
    await Promise.all([bump(), bump(), bump()]);
    assert.equal((await store.get('meta', 'c')).n, 3);
  });

  it('shows every transaction to the beforeCommit hook before it commits', async () => {
    const store = scratchStore();
    store.beforeCommit = async (tx) => {
      for (const { after } of tx.changes('meta')) after.seen = true;
    };
    await store.transact(async (tx) => void tx.put('meta', 'm', { id: 'm' }));
    assert.equal((await store.get('meta', 'm')).seen, true);
  });

  it('never calls the backend’s commit for a read-only transaction', async () => {
    const store = scratchStore();
    const commit = mock.method(store.backend, 'commit');
    assert.equal(await store.transact((tx) => tx.get('meta', 'm')), null);
    assert.equal(commit.mock.callCount(), 0);
  });

  it('reads and prunes the event log through the backend', async () => {
    const store = scratchStore();
    await store.transact(async (tx) => tx.emit('p1', 'old'));
    assert.equal((await store.events({ project: 'p1' })).length, 1);
    await store.prune(Date.now(), { p1: Date.now() + 1 });
    assert.deepEqual(await store.events({ project: 'p1' }), []);
  });

  it('admits and settles commands through the backend’s gate', async () => {
    const store = scratchStore();
    assert.equal(await store.beginCommand('none', null, 'i'), null);
    await store.finishCommand('none', null);
  });

  it('closes once queued transactions have finished', async () => {
    const store = scratchStore();
    const pending = store.transact(async (tx) => void tx.put('meta', 'm', { id: 'm' }));
    await store.close();
    await pending;
    assert.throws(() => store.backend.load([{ kind: 'meta', id: 'm' }]));
  });
});

/** An rpc client over one in-memory row, whose commit answers from `outcomes` in turn. */
function fakeRemote(outcomes: Array<'ok' | 'conflict'>) {
  const calls = [];
  return {
    calls,
    rpc: async (name, args) => {
      calls.push(name);
      if (name === 'control_load') return { data: args.queries.map(() => []) };
      const outcome = outcomes.shift() ?? 'conflict';
      return outcome === 'ok' ? { data: { ok: true, events: [] } } : { error: { message: 'control_conflict' } };
    },
  };
}

describe('ControlStore over a remote backend', () => {
  it('re-runs a transaction that hit a version conflict until it commits', async () => {
    mock.method(Math, 'random', () => 0);
    const remote = fakeRemote(['conflict', 'ok']);
    let runs = 0;
    const result = await new ControlStore({ remote }).transact(async (tx) => {
      runs++;
      tx.put('meta', 'm', { id: 'm' });
      return 'done';
    });
    assert.equal(result, 'done');
    assert.equal(runs, 2);
  });

  it('gives up with 503 storage_contention after the last attempt', async () => {
    mock.method(Math, 'random', () => 0);
    const remote = fakeRemote([]);
    const store = new ControlStore({ remote });
    await assert.rejects(
      store.transact(async (tx) => void tx.put('meta', 'm', { id: 'm' })),
      { code: 'storage_contention', status: 503 },
    );
    assert.equal(remote.calls.filter((c) => c === 'control_commit').length, MAX_TX_ATTEMPTS);
  });

  it('runs remote transactions concurrently rather than queueing them', async () => {
    const store = new ControlStore({ remote: fakeRemote(['ok', 'ok']) });
    assert.equal(store.serial, false);
    const results = await Promise.all(
      [1, 2].map((n) => store.transact(async (tx) => (tx.put('meta', `m${n}`, {}), n))),
    );
    assert.deepEqual(results, [1, 2]);
  });
});
