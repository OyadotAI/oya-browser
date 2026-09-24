/**
 * Personas round-trip through the Postgres driver (a stand-in for the pool),
 * not a local file: only known columns are written, and identity,
 * ownership, capacity, timestamps and device preferences come back unchanged.
 */
import assert from 'node:assert/strict';
import { mkdtemp } from 'node:fs/promises';
import { mock } from 'node:test';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import pg from 'pg';

process.env.OYA_DATA_DIR = await mkdtemp(join(tmpdir(), 'oya-persona-db-'));
process.env.OYA_STORAGE = 'postgres';
process.env.DATABASE_URL = 'postgres://stand-in/oya';
process.env.OYA_PROFILE_SECRET = 'b'.repeat(64);
let stored = [];
const columns = new Set([
  'id',
  'owner',
  'name',
  'seed',
  'prefs',
  'proxy',
  'device',
  'max_concurrent',
  'is_default',
  'created_at',
  'last_used_at',
  'updated_at',
]);

/** A bound parameter by its $n placeholder. */
const param = (params, placeholder) => params[Number(placeholder.slice(1)) - 1];

/** An upsert's rows, from its column list and its ($n, …) tuples, each replacing any stored row with its id. */
function upsert(sql, params) {
  const names = /\(([^)]*)\) values/.exec(sql)[1].split(', ');
  for (const column of names) assert.ok(columns.has(column), `Unknown database column: ${column}`);
  for (const tuple of sql.match(/\((\$\d+(?:, \$\d+)*)\)/g)) {
    const row = Object.fromEntries(
      tuple
        .slice(1, -1)
        .split(', ')
        .map((p, i) => [names[i], param(params, p)]),
    );
    stored = [...stored.filter((r) => r.id !== row.id), row];
  }
  return [];
}

/** A statement on personas: an upsert records its rows, a read returns them as Postgres would. */
const personaTable = (sql, params) => (sql.startsWith('insert') ? upsert(sql, params) : stored.map((r) => ({ ...r })));

mock.method(pg.Pool.prototype, 'query', async (sql, params = []) => ({
  rows: sql.includes('oya_browser.personas') ? personaTable(sql, params) : [],
}));
const { personas } = (await import('../../src/app/container.ts')).container;
const key = 'isolated-owner';
const defaultPersona = personas.defaultFor(key);
const custom = personas.create(key, {
  name: 'Stable device',
  maxConcurrent: 3,
  prefs: { platform: 'MacIntel', timezone: 'Pacific/Honolulu' },
});
const before = structuredClone([defaultPersona, custom]);
await personas.drain();
assert.equal(stored.length, 2, 'Both profiles must reach the database, not the file fallback');
assert.equal(stored[0].is_default, true);
assert.equal(stored[1].max_concurrent, 3);
personas.reset();
await personas.restore();
for (const profile of before) assert.deepEqual(personas.get(key, profile.id), profile);
assert.equal(personas.get('another-owner', custom.id), null);
console.log(
  'Profile database round-trip passed: identity, ownership, capacity, timestamps and device preferences preserved.',
);
