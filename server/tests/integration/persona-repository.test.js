/** Personas must survive a database outage: writes and reads fall back to the file. */
import assert from 'node:assert/strict';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { FilePersonaRepository, FallbackPersonaRepository, PersonaService } from '../../src/modules/personas/index.ts';

const file = new FilePersonaRepository(join(await mkdtemp(join(tmpdir(), 'oya-persona-repo-')), 'personas.json'));
const down = {
  loadAll: async () => {
    throw new Error('database unreachable');
  },
  saveAll: async () => {
    throw new Error('database unreachable');
  },
};
const repo = new FallbackPersonaRepository(down, file);
const quiet = console.error;
console.error = () => {};

assert.deepEqual(await repo.loadAll(), [], 'an empty store is an empty list');
const service = (repository) =>
  new PersonaService({
    repository,
    ownerOf: (key) => `owner:${key}`,
    proxies: { assigned: () => null, residential: () => null },
    mfa: { describe: () => null, list: () => [], clearAll() {} },
    credentials: { list: () => [], clearAll() {} },
    logins: { summary: () => null, clear() {} },
    metrics: { personaCapped: { inc() {} }, personasActive: { set() {} } },
  });
const writer = service(repo);
const made = writer.create('k', { name: 'Uncapped', maxConcurrent: 1 });
writer.update('k', made.id, { maxConcurrent: null });
await writer.drain();

const reader = service(repo);
await reader.restore();
console.error = quiet;
assert.deepEqual(reader.get('k', made.id), made, 'the persona round-trips through the file while the database is down');
assert.equal(reader.get('k', made.id).maxConcurrent, Infinity, 'an uncapped persona stays uncapped');
assert.equal(reader.get('someone-else', made.id), null, 'ownership survives storage');
console.log(
  'Persona repository fallback passed: database down on write and read, file round-trip, uncapped cap, ownership.',
);
