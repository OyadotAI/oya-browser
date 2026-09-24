/**
 * Personas survive a restart through storage: a service writes them to the
 * configured driver (SQLite in the scratch data directory here), a fresh
 * service restores them with the same seed, cap and owner, and a save that
 * storage refuses stays pending and lands on the next flush.
 */
import assert from 'node:assert/strict';
import { mock } from 'node:test';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

process.env.OYA_DATA_DIR = await mkdtemp(join(tmpdir(), 'oya-persona-repo-'));
const { PersonaService, PersonaTable } = await import('../../src/modules/personas/index.ts');
const { getConnection } = await import('../../src/platform/storage/index.ts');

/** A persona service over the storage-backed table, with inert collaborators. */
const service = () =>
  new PersonaService({
    repository: new PersonaTable(join(process.env.OYA_DATA_DIR, 'personas.json')),
    ownerOf: (key) => `owner:${key}`,
    proxies: { assigned: () => null, residential: () => null },
    mfa: { describe: () => null, list: () => [], clearAll: async () => 0 },
    credentials: { list: () => [], clearAll: async () => 0 },
    logins: { summary: () => null, clear() {} },
    metrics: { personaCapped: { inc() {} }, personasActive: { set() {} } },
  });

const writer = service();
const made = writer.create('k', { name: 'Uncapped', maxConcurrent: 1 });
writer.update('k', made.id, { maxConcurrent: null });
await writer.drain();

const reader = service();
await reader.restore();
assert.deepEqual(reader.get('k', made.id), made, 'the persona round-trips through storage');
assert.equal(reader.get('k', made.id).seed, made.seed, 'its seed, and so its device, is unchanged');
assert.equal(reader.get('k', made.id).maxConcurrent, Infinity, 'an uncapped persona stays uncapped');
assert.equal(reader.get('someone-else', made.id), null, 'ownership survives storage');

// Storage refuses the first save: the change stays pending, and the next flush stores it.
const quiet = console.error;
console.error = () => {};
const upsert = getConnection().upsert.bind(getConnection());
let refused = 0;
mock.method(getConnection(), 'upsert', async (...args) => {
  if (!refused++) throw new Error('storage unreachable');
  return upsert(...args);
});
const later = reader.create('k', { name: 'Saved late' });
await reader.drain();
assert.equal((await getConnection().select('personas', { id: later.id })).length, 0, 'the refused save stored nothing');
await reader.drain();
console.error = quiet;
mock.restoreAll();
assert.equal((await getConnection().select('personas', { id: later.id })).length, 1, 'the next flush stores it');

console.log('Persona storage passed: restart round-trip, seed and uncapped cap kept, ownership, refused save retried.');
