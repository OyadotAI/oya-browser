/**
 * Unit tests for the Postgres RPC adapter: control functions called by name
 * with named arguments, objects sent as JSON, one value back in Supabase's
 * { data } / { error } shape. The pool's query is stubbed, so nothing connects.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import pg from 'pg';
import { closePgPool, pgRemote } from '../../../../src/modules/control/pg-client.ts';

afterEach(async () => {
  mock.restoreAll();
  await closePgPool();
});

/** Stubs the pool's query, recording the SQL and parameters. */
function stubQuery(answer: (sql: string) => any) {
  const calls = [];
  mock.method(pg.Pool.prototype, 'query', async (sql, params) => (calls.push({ sql, params }), answer(sql)));
  return calls;
}

describe('pgRemote', () => {
  it('is null without a connection string', () => {
    assert.equal(pgRemote(''), null);
  });

  it('calls the control function by name with named arguments, sending objects as JSON', async () => {
    const calls = stubQuery(() => ({ rows: [{ control_commit: { ok: true } }] }));
    const remote = pgRemote('postgres://unused/db');
    const result = await remote.rpc('control_commit', { writes: [{ id: 'a' }], events: [], lim: 5, x: null });
    assert.deepEqual(result, { data: { ok: true } });
    assert.equal(calls[0].sql, 'select oya_browser.control_commit(writes => $1, events => $2, lim => $3, x => $4)');
    assert.deepEqual(calls[0].params, ['[{"id":"a"}]', '[]', 5, null]);
  });

  it('answers null data when the function returns no row or no value', async () => {
    stubQuery(() => ({ rows: [] }));
    assert.deepEqual(await pgRemote('postgres://unused/db').rpc('control_prune'), { data: null });
    mock.restoreAll();
    stubQuery(() => ({ rows: [{ control_prune: undefined }] }));
    assert.deepEqual(await pgRemote('postgres://unused/db').rpc('control_prune', {}), { data: null });
  });

  it('passes a failure’s message through as the error, so conflicts stay recognisable', async () => {
    stubQuery(() => {
      throw new Error('control_conflict');
    });
    assert.deepEqual(await pgRemote('postgres://unused/db').rpc('control_commit', {}), {
      error: { message: 'control_conflict' },
    });
  });
});
