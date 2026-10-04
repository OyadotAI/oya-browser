/**
 * The Postgres driver's statements and conversions, against a stubbed pool so
 * nothing connects: tables in the oya_browser schema, $n parameters with
 * objects sent as JSON, pg's own types turned back into what callers expect,
 * and control functions called by name in the { data } / { error } shape.
 * control-pg-client.test.js runs the same driver against a real Postgres.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import pg from 'pg';
import {
  PostgresConnection,
  pgRemote,
  closePgPool,
  libpqSslModes,
  pgConnectionString,
  verifyFull,
} from '../../../../src/platform/storage/postgres.ts';

afterEach(async () => {
  mock.restoreAll();
  await closePgPool();
});

/** Stubs the pool's query, recording each statement and answering with `answer(sql)`. */
function stubQuery(answer: (sql: string) => any[] = () => []) {
  const calls: { sql: string; params: unknown[] }[] = [];
  mock.method(pg.Pool.prototype, 'query', async (sql: string, params: unknown[]) => {
    calls.push({ sql, params });
    return { rows: answer(sql) };
  });
  return calls;
}

/** A driver on a database that is never reached. */
const db = () => new PostgresConnection('postgres://unused/db');

describe('PostgresConnection', () => {
  it('selects from the oya_browser schema with $n parameters, bounds, order and limit', async () => {
    const calls = stubQuery();
    await db().select('usage', { api_key: 'k', hour: { gte: 'h' } }, { order: ['hour', 'desc'], limit: 5 });
    assert.equal(
      calls[0].sql,
      'select * from oya_browser.usage where api_key = $1 and hour >= $2 order by hour desc limit $3',
    );
    assert.deepEqual(calls[0].params, ['k', 'h', 5]);
  });

  it('turns bigints, timestamps and json back into numbers, ISO strings and objects', async () => {
    const at = new Date('2026-09-01T00:00:00.000Z');
    stubQuery(() => [{ id: 'a', seed: '4294967295', created_at: at, prefs: { a: 1 }, is_default: true }]);
    const [row] = await db().select('personas');
    assert.deepEqual(row, {
      id: 'a',
      seed: 4294967295,
      created_at: '2026-09-01T00:00:00.000Z',
      prefs: { a: 1 },
      is_default: true,
    });
  });

  it('upserts every row in one statement, sending objects and arrays as JSON', async () => {
    const calls = stubQuery();
    await db().upsert(
      'personas',
      [
        { id: 'a', prefs: { list: [1] } },
        { id: 'b', prefs: null },
      ],
      { update: true },
    );
    assert.equal(
      calls[0].sql,
      'insert into oya_browser.personas (id, prefs) values ($1, $2), ($3, $4) on conflict (id) do update set prefs = excluded.prefs',
    );
    assert.deepEqual(calls[0].params, ['a', '{"list":[1]}', 'b', null]);
  });

  it('leaves stored rows alone on a plain insert', async () => {
    const calls = stubQuery();
    await db().upsert('api_keys', [{ key_hash: 'h' }]);
    assert.match(calls[0].sql, / on conflict \(key_hash\) do nothing$/);
  });

  it('writes nothing for no rows', async () => {
    const calls = stubQuery();
    await db().upsert('personas', []);
    assert.equal(calls.length, 0);
  });

  it('counts the rows an update or delete touched', async () => {
    const calls = stubQuery(() => [{}, {}]);
    assert.equal(await db().update('api_keys', { key_hash: 'h', user_id: null }, { user_id: 'u' }), 2);
    assert.equal(
      calls[0].sql,
      'update oya_browser.api_keys set user_id = $1 where key_hash = $2 and user_id is null returning 1',
    );
    assert.equal(await db().delete('api_keys', { key_hash: { notIn: ['x', 'y'] } }), 2);
    assert.equal(calls[1].sql, 'delete from oya_browser.api_keys where key_hash not in ($1, $2) returning 1');
  });

  it('asks the catalog whether a legacy table exists', async () => {
    const calls = stubQuery(() => [{ found: false }]);
    assert.equal(await db().exists('browsers'), false);
    assert.deepEqual(calls[0].params, ['oya_browser.browsers']);
  });

  it('gives the control plane its remote client', () => {
    assert.ok(db().controlRemote()?.rpc);
  });
});

describe('pgRemote', () => {
  it('is null without a connection string', () => {
    assert.equal(pgRemote(''), null);
  });

  it('calls a control function by name with named arguments, sending objects as JSON', async () => {
    const calls = stubQuery(() => [{ control_commit: { ok: true } }]);
    const result = await pgRemote('postgres://unused/db').rpc('control_commit', { writes: [{ id: 'a' }], lim: 5 });
    assert.deepEqual(result, { data: { ok: true } });
    assert.equal(calls[0].sql, 'select oya_browser.control_commit(writes => $1, lim => $2)');
    assert.deepEqual(calls[0].params, ['[{"id":"a"}]', 5]);
  });

  it('answers null data when the function returns no row', async () => {
    stubQuery(() => []);
    assert.deepEqual(await pgRemote('postgres://unused/db').rpc('control_prune'), { data: null });
  });

  it('passes a failure’s message through as the error, so a conflict stays recognisable', async () => {
    mock.method(pg.Pool.prototype, 'query', async () => {
      throw new Error('control_conflict');
    });
    assert.deepEqual(await pgRemote('postgres://unused/db').rpc('control_commit', {}), {
      error: { message: 'control_conflict' },
    });
  });
});

describe('libpqSslModes', () => {
  it('reads sslmode as libpq does, so require encrypts without refusing a provider CA', () => {
    assert.equal(
      libpqSslModes('postgres://h/db?sslmode=require'),
      'postgres://h/db?sslmode=require&uselibpqcompat=true',
    );
    assert.equal(libpqSslModes('postgres://h/db'), 'postgres://h/db?uselibpqcompat=true');
  });

  it('leaves a string that already chose its semantics alone', () => {
    const chosen = 'postgres://h/db?uselibpqcompat=false&sslmode=verify-full';
    assert.equal(libpqSslModes(chosen), chosen);
  });
});

/** The TLS options node-postgres derives from a connection string, as a client would dial with. */
const sslOf = (url: string) => (new pg.Client({ connectionString: url }) as any).connectionParameters.ssl;

describe('pgConnectionString', () => {
  it('keeps the libpq reading by default, so a local URL without TLS still connects', () => {
    const local = 'postgresql://postgres:pw@127.0.0.1:54322/postgres';
    assert.equal(pgConnectionString(local, undefined), `${local}?uselibpqcompat=true`);
    assert.equal(sslOf(pgConnectionString(local, 'libpq')), false);
  });

  it('under verify-full, requires TLS and checks the certificate even when the URL asked for less', () => {
    for (const url of ['postgres://h/db', 'postgres://h/db?sslmode=disable', 'postgres://h/db?sslmode=require']) {
      const ssl = sslOf(pgConnectionString(url, 'verify-full'));
      assert.equal(typeof ssl, 'object', url);
      assert.notEqual(ssl.rejectUnauthorized, false, url);
      assert.equal(ssl.checkServerIdentity, undefined, url);
    }
  });

  it('refuses an OYA_DB_TLS it does not know, rather than falling back to plaintext', () => {
    assert.throws(() => pgConnectionString('postgres://h/db', 'verify'), /OYA_DB_TLS must be libpq or verify-full/);
  });
});

describe('verifyFull', () => {
  it('replaces a weaker sslmode and drops libpq compatibility, keeping the other parameters', () => {
    assert.equal(
      verifyFull('postgres://u:p@h:5432/db?sslmode=require&uselibpqcompat=true&application_name=oya', ''),
      'postgres://u:p@h:5432/db?sslmode=verify-full&application_name=oya',
    );
  });

  it('takes the CA from PGSSLROOTCERT when the URL names none', () => {
    assert.equal(
      verifyFull('postgres://h/db', '/etc/ssl/ca.pem'),
      'postgres://h/db?sslmode=verify-full&sslrootcert=%2Fetc%2Fssl%2Fca.pem',
    );
  });

  it('keeps a CA the URL already names', () => {
    assert.equal(
      verifyFull('postgres://h/db?sslrootcert=/a.pem', '/b.pem'),
      'postgres://h/db?sslrootcert=%2Fa.pem&sslmode=verify-full',
    );
  });
});
