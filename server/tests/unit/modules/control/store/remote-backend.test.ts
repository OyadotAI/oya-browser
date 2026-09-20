/**
 * Unit tests for RemoteBackend: each storage call maps to its RPC, and RPC
 * failures come back as the same coded errors the SQLite backend throws.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { RemoteBackend } from '../../../../../src/modules/control/store.ts';

/** A client whose rpc() records its calls and answers with `answer`. */
function client(answer: any = { data: null }) {
  const calls = [];
  return { calls, rpc: async (name, args) => (calls.push({ name, args }), answer) };
}

describe('RemoteBackend calls', () => {
  it('loads through control_load', async () => {
    const c = client({ data: [[{ id: 'a' }]] });
    assert.deepEqual(await new RemoteBackend(c).load([{ kind: 'session', id: 'a' }]), [[{ id: 'a' }]]);
    assert.deepEqual(c.calls[0], { name: 'control_load', args: { queries: [{ kind: 'session', id: 'a' }] } });
  });

  it('commits through control_commit, defaulting to no writes or events', async () => {
    const c = client({ data: { ok: true, events: [] } });
    assert.deepEqual(await new RemoteBackend(c).commit({}), { ok: true, events: [] });
    assert.deepEqual(c.calls[0].args, { writes: [], events: [] });
  });

  it('answers a version conflict with ok: false', async () => {
    const c = client({ error: { message: 'ERROR: control_conflict' } });
    assert.deepEqual(await new RemoteBackend(c).commit({ writes: [] }), { ok: false });
  });

  it('reads events with the documented defaults', async () => {
    const c = client({ data: [] });
    await new RemoteBackend(c).events();
    assert.deepEqual(c.calls[0], {
      name: 'control_read_events',
      args: { target_project: null, after_seq: 0, lim: 500, latest: false, seqs: null },
    });
  });

  it('prunes, begins and finishes commands through their RPCs', async () => {
    const c = client({ data: 7 });
    const backend = new RemoteBackend(c);
    await backend.prune(5);
    assert.equal(await backend.beginCommand('s', 'h', 'i'), 7);
    await backend.finishCommand('s', 7);
    assert.deepEqual(c.calls, [
      { name: 'control_prune', args: { now_ms: 5, cutoffs: {} } },
      { name: 'control_begin', args: { session_id: 's', actor: 'h', caller_instance: 'i' } },
      { name: 'control_finish', args: { session_id: 's', generation: 7 } },
    ]);
  });

  it('makes no call to finish a command that was never admitted', async () => {
    const c = client();
    await new RemoteBackend(c).finishCommand('s', null);
    assert.equal(c.calls.length, 0);
  });

  it('has nothing to close', () => {
    assert.doesNotThrow(() => new RemoteBackend(client()).close());
  });
});

describe('RemoteBackend errors', () => {
  /** What beginCommand rejects with when the RPC fails with `message`. */
  const failure = (message) => new RemoteBackend(client({ error: { message } })).beginCommand('s', null, 'i');

  it('turns commands_pending into a 409 with that code', async () => {
    await assert.rejects(failure('commands_pending'), { status: 409, code: 'commands_pending' });
  });

  it('turns control_paused into a 409 with that code', async () => {
    await assert.rejects(failure('control_paused'), { status: 409, code: 'control_paused' });
  });

  it('turns anything else into 503 storage_unavailable, keeping the cause', async () => {
    const error = { message: 'connection reset' };
    await assert.rejects(new RemoteBackend(client({ error })).load([]), (e: any) => {
      assert.equal(e.status, 503);
      assert.equal(e.code, 'storage_unavailable');
      assert.equal(e.cause, error);
      return true;
    });
  });

  it('treats an error without a message as unavailable storage', async () => {
    await assert.rejects(new RemoteBackend(client({ error: {} })).load([]), { code: 'storage_unavailable' });
  });
});
