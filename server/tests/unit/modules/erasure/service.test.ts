/**
 * Unit tests for erasure: a deleted project is left alone during its grace
 * period, forgotten on every replica after it, erased from storage once that
 * has settled (unless a browser or recording still needs it), and cut to a
 * tombstone; an account deletion deletes the person's projects, memberships,
 * keys, profile and sign-in, refused through "Login as" or while billing.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-erasure-');
const { Erasure, PURGE_GRACE_MS, PURGE_SETTLE_MS } = await import('../../../../src/modules/erasure/index.ts');
const { scratchStore } = await import('../../support/control.ts');

const OWNER = 'aaaa1111bbbb2222';
const ID = 'prj_one';
const NOW = 10 * PURGE_GRACE_MS;
const SETTLED = NOW - PURGE_GRACE_MS - PURGE_SETTLE_MS;

let store, calls, deps, erasure;

/** Writes a control row as-is. */
const put = (kind, id, body) =>
  store.transact(async (tx) => {
    await tx.get(kind, id);
    tx.put(kind, id, body);
  });

/** A project deleted at `deletedAt`. */
const deleted = (deletedAt, extra = {}) =>
  put('project', ID, { id: ID, legacyOwner: OWNER, ownerUser: 'u1', name: 'P', key: 'sealed', deletedAt, ...extra });

/** Records each call to a fake collaborator under `name`. */
const record =
  (name, answer?) =>
  async (...args) => {
    calls.push([name, ...args.map((a) => (a instanceof Set ? [...a] : a))]);
    return answer;
  };

beforeEach(() => {
  store = scratchStore();
  calls = [];
  deps = {
    store: () => store,
    rows: {
      deleteProjectRows: record('rows'),
      deleteUserRows: record('userRows'),
      subscriptionOf: async () => null,
      deleteSignIn: record('signIn'),
    },
    personas: { forgetOwners: (o) => calls.push(['personas', [...o]]) },
    settings: { forgetOwners: record('settings') },
    usage: { forget: (o) => calls.push(['usage', [...o]]) },
    profiles: { removeOwners: record('profiles', 0) },
    deleteProject: async (userId, id) => {
      calls.push(['deleteProject', userId, id]);
      const tx = await store.get('project', id);
      await put('project', id, { ...tx, deletedAt: NOW });
    },
    audit: (e) => calls.push(['audit', e.action, e.targetId]),
    now: () => NOW,
  };
  erasure = new Erasure(deps);
});

/** Names of the collaborators called, in order. */
const names = () => calls.map((c) => c[0]);

describe('erasure maintenance', () => {
  it('leaves a project alone during its grace period', async () => {
    await deleted(NOW - PURGE_GRACE_MS + 1);
    await erasure.maintain();
    assert.deepEqual(calls, []);
  });

  it('leaves a live project alone', async () => {
    await put('project', ID, { id: ID, legacyOwner: OWNER });
    await erasure.maintain();
    assert.deepEqual(calls, []);
  });

  it('forgets a past-grace project on this replica before storage is erased', async () => {
    await deleted(NOW - PURGE_GRACE_MS);
    await erasure.maintain();
    assert.deepEqual(names(), ['personas', 'usage', 'settings', 'profiles']);
    assert.deepEqual(calls[0], ['personas', [OWNER]]);
    assert.equal((await store.get('project', ID)).key, 'sealed');
  });

  it('erases a settled project from storage and the control plane, keeping a tombstone', async () => {
    await deleted(SETTLED);
    await put('webhook', 'w1', { id: 'w1', project: ID });
    await put('membership', 'm1', { id: 'm1', project: ID, userId: 'u2' });
    await put('session', 's1', { id: 's1', project: ID, state: 'stopped' });
    await put('webhook', 'w2', { id: 'w2', project: 'prj_other' });
    await erasure.maintain();
    assert.deepEqual(
      calls.find((c) => c[0] === 'rows'),
      ['rows', OWNER, ID],
    );
    for (const [kind, id] of [
      ['webhook', 'w1'],
      ['membership', 'm1'],
      ['session', 's1'],
    ])
      assert.equal(await store.get(kind, id), null, `${kind} ${id}`);
    assert.ok(await store.get('webhook', 'w2'));
    assert.deepEqual(await store.get('project', ID), {
      id: ID,
      legacyOwner: OWNER,
      deletedAt: SETTLED,
      purgedAt: NOW,
      settings: {},
    });
    assert.deepEqual(calls.at(-1), ['audit', 'project.purge', ID]);
  });

  it('does not touch a project already erased', async () => {
    await deleted(SETTLED, { purgedAt: NOW - 1 });
    await erasure.maintain();
    assert.deepEqual(calls, []);
  });

  it('waits while a browser of the project still needs cleaning up', async () => {
    await deleted(SETTLED);
    await put('session', 's1', { id: 's1', project: ID, state: 'cleanup_pending' });
    await erasure.maintain();
    assert.equal(names().includes('rows'), false);
    assert.equal((await store.get('project', ID)).key, 'sealed');
  });

  it('waits while a recording of the project is still archived', async () => {
    await deleted(SETTLED);
    await put('recording', 'r1', { sessionId: 'r1', owner: OWNER });
    await erasure.maintain();
    assert.equal(names().includes('rows'), false);
  });

  it('leaves the project to the next pass when erasing fails', async () => {
    await deleted(SETTLED);
    deps.rows.deleteProjectRows = async () => {
      throw new Error('db down');
    };
    await assert.rejects(erasure.maintain(), /db down/);
    assert.equal((await store.get('project', ID)).purgedAt, undefined);
  });
});

describe('account deletion', () => {
  it("deletes the person's projects, memberships, keys, profile and sign-in", async () => {
    await put('project', ID, { id: ID, legacyOwner: OWNER, ownerUser: 'u1' });
    await put('project', 'prj_gone', { id: 'prj_gone', legacyOwner: 'x', ownerUser: 'u1', deletedAt: 1 });
    await put('membership', 'm9', { id: 'm9', project: 'prj_theirs', userId: 'u1' });
    assert.deepEqual(await erasure.deleteAccount('u1'), { ok: true, projects: 1 });
    assert.deepEqual(names(), ['deleteProject', 'userRows', 'signIn']);
    assert.deepEqual(calls[0], ['deleteProject', 'u1', ID]);
    assert.equal(await store.get('membership', 'm9'), null);
  });

  it('refuses a deletion through Login as', async () => {
    await assert.rejects(erasure.deleteAccount('u1', 'admin-1'), (e: any) => e.status === 403);
    assert.deepEqual(calls, []);
  });

  it('refuses while a subscription still bills', async () => {
    deps.rows.subscriptionOf = async () => ({ status: 'active' });
    await assert.rejects(erasure.deleteAccount('u1'), (e: any) => e.status === 409);
    assert.deepEqual(calls, []);
  });

  it('allows it once the subscription is cancelled', async () => {
    deps.rows.subscriptionOf = async () => ({ status: 'canceled' });
    assert.deepEqual(await erasure.deleteAccount('u1'), { ok: true, projects: 0 });
  });
});
