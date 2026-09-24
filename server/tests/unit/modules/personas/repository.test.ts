/**
 * Unit tests for persona storage: the personas table in the configured storage
 * (the scratch directory's SQLite file here), the one-time import of a
 * personas.json left from before storage drivers, and explicit deletion.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-persona-repo-');
const { PersonaTable } = await import('../../../../src/modules/personas/index.ts');
const { shape } = await import('../../../../src/modules/personas/model.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

/** Where a legacy personas.json would sit. */
const LEGACY = join(dir, 'personas.json');
/** A seed far from any default, so a re-rolled device would show. */
const SEED = 987654321;

/** A persona as the service holds it. */
const persona = (id: string, extra: object = {}) =>
  shape({
    id,
    owner: 'o',
    name: id,
    seed: 7,
    prefs: { platform: 'Win32' },
    createdAt: '2026-01-01T00:00:00.000Z',
    ...extra,
  });

/** Writes a legacy personas.json holding these personas, uncapped written as null. */
const writeLegacy = (personas: object[]) => writeFileSync(LEGACY, JSON.stringify(personas));

describe('PersonaTable', () => {
  beforeEach(async () => {
    await getConnection().delete('personas', {});
  });

  it('reads an empty table as no personas', async () => {
    assert.deepEqual(await new PersonaTable(LEGACY).loadAll(), []);
  });

  it('round-trips personas, an uncapped one as Infinity, with seed, prefs and timestamps intact', async () => {
    const repo = new PersonaTable(LEGACY);
    const uncapped = persona('a', { maxConcurrent: Infinity, isDefault: true, seed: SEED });
    await repo.saveAll([uncapped, persona('b', { maxConcurrent: 3 })]);
    const byId = Object.fromEntries((await repo.loadAll()).map((p) => [p.id, p]));
    assert.deepEqual(byId.a, uncapped);
    assert.deepEqual(byId.b, persona('b', { maxConcurrent: 3 }));
  });

  it('stores an uncapped persona as a null cap', async () => {
    await new PersonaTable(LEGACY).saveAll([persona('a', { maxConcurrent: Infinity })]);
    const [row] = await getConnection().select('personas', { id: 'a' });
    assert.equal(row.max_concurrent, null);
  });

  it('upserts: saving a persona again replaces its stored row', async () => {
    const repo = new PersonaTable(LEGACY);
    await repo.saveAll([persona('a')]);
    await repo.saveAll([persona('a', { name: 'Renamed' })]);
    assert.deepEqual(
      (await repo.loadAll()).map((p) => p.name),
      ['Renamed'],
    );
  });

  it('never deletes a persona a save leaves out, such as one another replica made', async () => {
    const repo = new PersonaTable(LEGACY);
    await repo.saveAll([persona('a'), persona('b')]);
    await repo.saveAll([persona('a')]);
    assert.equal((await repo.loadAll()).length, 2);
  });

  it('removes exactly the ids it is given', async () => {
    const repo = new PersonaTable(LEGACY);
    await repo.saveAll([persona('a'), persona('b'), persona('c')]);
    await repo.remove(['a', 'c']);
    assert.deepEqual(
      (await repo.loadAll()).map((p) => p.id),
      ['b'],
    );
  });

  it('imports a legacy personas.json on first read, seeds intact, and sets the file aside', async () => {
    writeLegacy([{ ...persona('old', { seed: SEED }), maxConcurrent: null }]);
    const [p] = await new PersonaTable(LEGACY).loadAll();
    assert.deepEqual([p.id, p.seed, p.maxConcurrent], ['old', SEED, Infinity]);
    assert.equal(existsSync(LEGACY), false);
    assert.equal(existsSync(`${LEGACY}.imported`), true);
  });

  it('keeps a stored persona over the stale copy in a legacy file', async () => {
    const repo = new PersonaTable(LEGACY);
    await repo.saveAll([persona('a', { name: 'Stored' })]);
    writeLegacy([{ ...persona('a', { name: 'Stale' }), maxConcurrent: null }]);
    assert.deepEqual(
      (await repo.loadAll()).map((p) => p.name),
      ['Stored'],
    );
  });

  it('throws on a legacy file it cannot parse, rather than reading it as empty', async () => {
    writeFileSync(LEGACY, '{not json');
    await assert.rejects(new PersonaTable(LEGACY).loadAll(), SyntaxError);
    assert.equal(existsSync(LEGACY), true);
  });
});
