/**
 * Unit tests for persona group storage (personas/group-repository.ts) over the
 * scratch directory's SQLite file: a group round-trips with its members, a
 * second create of one name is refused, and a deleted persona leaves only the
 * groups that held it.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-persona-groups-');
const { GroupTable } = await import('../../../../src/modules/personas/index.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

const T = '2026-01-01T00:00:00.000Z';
/** A group of `owner`'s made at `at`. */
const group = (owner: string, name: string, personaIds: string[], at = T) => ({
  owner,
  name,
  personaIds,
  createdAt: at,
  updatedAt: at,
});

describe('GroupTable', () => {
  const table = new GroupTable();
  beforeEach(() => getConnection().delete('persona_groups', {}));

  it('stores a group and reads it back with its members, by owner and name', async () => {
    assert.equal(await table.insert(group('o', 'pool', ['p1', 'p2'])), true);
    assert.deepEqual(await table.get('o', 'pool'), group('o', 'pool', ['p1', 'p2']));
    assert.equal(await table.get('other', 'pool'), null);
    assert.deepEqual(
      (await table.list('o')).map((g) => g.name),
      ['pool'],
    );
  });

  it('refuses a second create of the same name, keeping the first one’s members', async () => {
    await table.insert(group('o', 'pool', ['p1']));
    assert.equal(await table.insert(group('o', 'pool', ['p2'], '2026-02-01T00:00:00.000Z')), false);
    assert.deepEqual((await table.get('o', 'pool'))?.personaIds, ['p1']);
  });

  it('replaces members and deletes, answering false for a group that is not there', async () => {
    await table.insert(group('o', 'pool', ['p1']));
    assert.equal(await table.replace('o', 'pool', ['p3']), true);
    assert.deepEqual((await table.get('o', 'pool'))?.personaIds, ['p3']);
    assert.equal(await table.replace('o', 'nope', []), false);
    assert.equal(await table.remove('o', 'pool'), true);
    assert.equal(await table.remove('o', 'pool'), false);
  });

  it('takes a deleted persona out of only the groups that held it', async () => {
    await table.insert(group('o', 'a', ['p1', 'p2']));
    await table.insert(group('o', 'b', ['p2']));
    await table.insert(group('o', 'c', ['p3']));
    await table.dropMember('o', 'p2');
    const members = Object.fromEntries((await table.list('o')).map((g) => [g.name, g.personaIds]));
    assert.deepEqual(members, { a: ['p1'], b: [], c: ['p3'] });
  });
});
