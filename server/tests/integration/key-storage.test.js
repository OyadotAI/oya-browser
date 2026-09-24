#!/usr/bin/env node
/**
 * The API key must never reach the database.
 *
 * api_keys used to store the bearer credential itself, so any read of that
 * table, a backup, a replica, a support export, an over-broad grant, handed
 * over working administrator credentials for every tenant. It now stores
 * sha256(key) plus an 8-character prefix.
 *
 * A unit test cannot catch a `where key = $1` that should have been
 * `where key_hash = $1`. So this stands a recording stand-in in place of the
 * Postgres pool, drives the real auth module, and searches every statement and
 * every parameter the server sent, and every row written, for the key in the clear.
 *
 * Usage: node key-storage.test.js
 */

import { createServer } from 'http';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mock } from 'node:test';
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import pg from 'pg';

// ── A Postgres stand-in that records everything it is asked ──

/** Every statement the server ran: { sql, params }. */
const seen = [];
let rows = [];
const controlRows = new Map();

/** The value bound to a control function's named argument. */
const arg = (sql, params, name) => {
  const value = params[Number(new RegExp(`${name} => \\$(\\d+)`).exec(sql)[1]) - 1];
  return typeof value === 'string' ? JSON.parse(value) : value;
};

/** control_load: the rows each query asks for. */
const load = (queries) =>
  queries.map((q) =>
    [...controlRows.values()].filter(
      (r) =>
        r.kind === q.kind &&
        (q.id === undefined || r.id === q.id) &&
        (q.project === undefined || r.project === q.project) &&
        (!q.states || q.states.includes(r.state)),
    ),
  );

/** control_commit: store each write with its version bumped. */
const commit = (writes) => {
  for (const w of writes)
    controlRows.set(`${w.kind}:${w.id}`, { ...w, version: (controlRows.get(`${w.kind}:${w.id}`)?.version || 0) + 1 });
  return { ok: true };
};

/** A control function call, answered as the one column Postgres returns. */
function controlCall(sql, params) {
  const name = /select oya_browser\.(\w+)\(/.exec(sql)[1];
  if (name === 'control_load') return [{ [name]: load(arg(sql, params, 'queries')) }];
  if (name === 'control_commit') return [{ [name]: commit(arg(sql, params, 'writes')) }];
  return [{ [name]: [] }];
}

/** A bound parameter by its $n placeholder. */
const param = (params, placeholder) => params[Number(placeholder.slice(1)) - 1];

/** Whether a row passes one `col = $n`, `col is null` or `col >= $n` test from a where clause. */
function passes(row, test, params) {
  const [, column, op, value] = /^(\w+) (=|>=|is null)\s*(\$\d+)?$/.exec(test.trim());
  if (op === 'is null') return row[column] == null;
  return op === '=' ? row[column] === param(params, value) : row[column] >= param(params, value);
}

/** The rows of `table` a statement's where clause selects. */
const matching = (sql, params) => {
  const where = / where (.*?)(?: order by| limit| returning|$)/.exec(sql)?.[1];
  return where ? rows.filter((r) => where.split(' and ').every((t) => passes(r, t, params))) : rows;
};

/** An insert's rows, from its column list and its ($n, …) tuples; an existing digest is left alone. */
function insert(sql, params) {
  const columns = /\(([^)]*)\) values/.exec(sql)[1].split(', ');
  for (const tuple of sql.match(/\((\$\d+(?:, \$\d+)*)\)/g)) {
    const row = Object.fromEntries(
      tuple
        .slice(1, -1)
        .split(', ')
        .map((p, i) => [columns[i], param(params, p)]),
    );
    if (!rows.some((r) => r.key_hash === row.key_hash)) rows.push(row);
  }
  return [];
}

/** An update's `set a = $n, …` applied to the rows it matches; one answer row per change, as `returning 1` gives. */
function update(sql, params) {
  const hits = matching(sql, params);
  for (const assignment of / set (.*?) where /.exec(sql)[1].split(', ')) {
    const [column, placeholder] = assignment.split(' = ');
    for (const row of hits) row[column] = param(params, placeholder);
  }
  return hits.map(() => ({ '?column?': 1 }));
}

/** A statement on api_keys, answered from what was written. */
function apiKeys(sql, params) {
  if (sql.startsWith('insert')) return insert(sql, params);
  if (sql.startsWith('update')) return update(sql, params);
  if (sql.startsWith('delete')) {
    const gone = matching(sql, params);
    rows = rows.filter((r) => !gone.includes(r));
    return gone.map(() => ({ '?column?': 1 }));
  }
  return matching(sql, params).map((r) => ({ ...r }));
}

mock.method(pg.Pool.prototype, 'query', async (sql, params = []) => {
  seen.push({ sql, params });
  if (/^select oya_browser\.control_/.test(sql)) return { rows: controlCall(sql, params) };
  return { rows: sql.includes('oya_browser.api_keys') ? apiKeys(sql, params) : [] };
});

// ── Supabase Auth, which only ever says who a bearer token is ──

const USER = '11111111-2222-3333-4444-555555555555';
const auth0 = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ id: req.headers.authorization === 'Bearer test-owner' ? USER : 'other-user' }));
});
const PORT = await new Promise((resolve) => auth0.listen(0, '127.0.0.1', () => resolve(auth0.address().port)));

process.env.OYA_STORAGE = 'postgres';
process.env.DATABASE_URL = 'postgres://stand-in/oya';
process.env.SUPABASE_URL = `http://127.0.0.1:${PORT}`;
process.env.SUPABASE_SERVICE_KEY = 'stub-service-key';
process.env.OYA_DATA_DIR = mkdtempSync(join(tmpdir(), 'oya-keystore-'));
process.env.OYA_PROFILE_SECRET = 'test-secret-for-key-storage';

const auth = await import('../../src/modules/auth/service.ts');
await auth.authReady;

const sha256 = (v) => createHash('sha256').update(v).digest('hex');

let passed = 0;
const check = (label, fn) => {
  fn();
  console.log(`  ✅ ${label}`);
  passed++;
};

// ── The key never appears on the wire ──

console.log('\n1️⃣  Registering a key sends only its digest');
const KEY = 'aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456';
// Exercise the digest row and encrypted control-store project together.
await auth.registerApiKey(KEY, USER, 'Test project');

check('a row was written', () => assert.equal(rows.length, 1));
check('it carries the digest, not the key', () => {
  assert.equal(rows[0].key_hash, sha256(KEY));
  assert.equal(rows[0].key, undefined);
});
check('it carries a prefix for display', () => assert.equal(rows[0].key_prefix, KEY.slice(0, 8)));
check('and the project the key opens', () => assert.match(rows[0].project, /^prj_[0-9a-f]{24}$/));

console.log('\n2️⃣  Nothing the server sent contains the key');
check(`${seen.length} statements, none carrying the key`, () => {
  for (const r of seen) {
    assert.ok(!r.sql.includes(KEY), `key in SQL: ${r.sql}`);
    assert.ok(!JSON.stringify(r.params).includes(KEY), `key in parameters: ${JSON.stringify(r.params)}`);
  }
});
check('and none of them names the old `key` column', () => {
  for (const r of seen) assert.ok(!/\bkey = \$/.test(r.sql), `looked up by plaintext: ${r.sql}`);
});

console.log('\n3️⃣  The key still authenticates, by digest');
const principal = await auth.authenticateToken(KEY);
check('a registered key is accepted', () => assert.equal(principal.role, 'administrator'));
check('and is still the credential downstream handlers get', () => assert.equal(principal.key, KEY));
await assert.rejects(auth.authenticateToken('never-registered-key-0123456789ab'), /Invalid API key/);
check('an unregistered key is refused', () => {});
check('validateApiKey works off the digest cache', () => {
  assert.equal(auth.validateApiKey(KEY), true);
  assert.equal(auth.validateApiKey('not-a-key'), false);
});

console.log('\n4️⃣  Listing keys returns metadata, never a key');
const listed = await auth.listApiKeys(USER);
check('one key is listed', () => assert.equal(listed.length, 1));
check('with an id, a prefix and a project', () => {
  assert.equal(listed[0].id, sha256(KEY));
  assert.equal(listed[0].prefix, KEY.slice(0, 8));
  assert.match(listed[0].project, /^prj_[0-9a-f]{24}$/);
});
check('and no key in the payload at all', () => {
  assert.ok(!JSON.stringify(listed).includes(KEY));
  assert.equal(listed[0].key, undefined);
});

const { control, projectId } = await import('../../src/modules/control/service.ts');
const project = projectId(KEY);
await control().store.transact(async (tx) => {
  (await tx.get('project', project)).key = 'broken';
});
assert.throws(() => control().projectKey(controlRows.get(`project:${project}`).body), {
  code: 'project_key_unavailable',
});
await auth.registerApiKey(KEY, USER, 'Test project');
check('reimporting the original key repairs the encrypted project credential', () =>
  assert.equal(control().projectKey(controlRows.get(`project:${project}`).body), KEY),
);
const express = (await import('express')).default;
const { projectAccountRouter } = await import('../../src/modules/control/membership.ts');
const app = express();
app.use(express.json());
app.use('/projects', projectAccountRouter);
const apiServer = app.listen(0, '127.0.0.1');
await new Promise((resolve) => apiServer.once('listening', resolve));
const endpoint = `http://127.0.0.1:${apiServer.address().port}/projects`;
const request = (path, method = 'GET', body, token = 'test-owner') =>
  fetch(endpoint + path, {
    method,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
assert.equal((await request(`/${project}/key`, 'POST', {}, 'other')).status, 404);
const copied = await request(`/${project}/key`, 'POST', {});
assert.equal(copied.headers.get('cache-control'), 'no-store');
assert.equal((await copied.json()).key, KEY);
assert.equal((await request(`/${project}`, 'PATCH', { name: 'New name' })).status, 200);
assert.equal((await (await request('')).json())[0].name, 'New name', 'old API key label must not override a rename');
assert.equal((await request(`/${project}`, 'DELETE', undefined, 'other')).status, 404);
assert.equal((await request(`/${project}`, 'DELETE')).status, 200);
assert.deepEqual(await (await request('')).json(), [], 'deleted project must disappear from the account list');
assert.equal((await request(`/${project}/key`, 'POST', {})).status, 404);
assert.equal((await request(`/${project}/access`, 'POST', {})).status, 404);
check(
  'account endpoints enforce ownership, copy without caching, persist names, and remove deleted projects',
  () => {},
);
apiServer.close();
await assert.rejects(auth.authenticateToken(KEY), { status: 410 });
await assert.rejects(auth.registerApiKey(KEY, USER), { status: 410 });
check('deleted projects reject raw keys and cannot be resurrected by import', () => {});

console.log('\n5️⃣  Deleting works by digest');
await auth.deleteApiKey(listed[0].id, USER);
check('the row is gone', () => assert.equal(rows.length, 0));
check('and the digest no longer validates', () => assert.equal(auth.validateApiKey(KEY), false));

console.log(`\n  ${passed} passed, 0 failed\n`);
auth0.close();
process.exit(0);
