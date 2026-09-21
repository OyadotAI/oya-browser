/**
 * Personas round-trip through the database (a stubbed Supabase REST endpoint), not
 * the file fallback: only known columns are written, and identity, ownership,
 * capacity, timestamps and device preferences come back unchanged.
 */
import assert from 'node:assert/strict';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

process.env.OYA_DATA_DIR = await mkdtemp(join(tmpdir(), 'oya-persona-db-'));
process.env.SUPABASE_URL = 'https://database.invalid';
process.env.SUPABASE_SERVICE_KEY = 'isolated-test-key';
process.env.OYA_PROFILE_SECRET = 'b'.repeat(64);
let stored = [];
const columns = new Set([
  'id',
  'owner',
  'name',
  'seed',
  'prefs',
  'proxy',
  'max_concurrent',
  'is_default',
  'created_at',
  'last_used_at',
  'updated_at',
]);
globalThis.fetch = async (url, init) => {
  assert.equal(new URL(url).pathname, '/rest/v1/personas');
  if (init.method === 'POST') {
    stored = JSON.parse(init.body);
    for (const row of stored)
      for (const key of Object.keys(row)) assert.ok(columns.has(key), `Unknown database column: ${key}`);
    return new Response(null, { status: 201 });
  }
  return Response.json(stored);
};
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
