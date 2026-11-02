/**
 * Unit tests for the persona group service (personas/groups.ts): who may be a
 * member, what each refusal says and carries as its code, and which members a
 * start may run as, in the order the reservation breaks ties in.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { GroupService } from '../../../../src/modules/personas/groups.ts';
import type { GroupRepository, PersonaGroup } from '../../../../src/modules/personas/group-repository.ts';

/** A repository in memory, keyed like the table. */
class MemoryGroups implements GroupRepository {
  /** Groups by owner and name. */
  rows = new Map<string, PersonaGroup>();
  /** The row key. */
  private at = (owner: string, name: string) => `${owner}/${name}`;
  async list(owner: string) {
    return [...this.rows.values()].filter((g) => g.owner === owner);
  }
  async get(owner: string, name: string) {
    return this.rows.get(this.at(owner, name)) ?? null;
  }
  async insert(g: PersonaGroup) {
    if (this.rows.has(this.at(g.owner, g.name))) return false;
    this.rows.set(this.at(g.owner, g.name), g);
    return true;
  }
  async replace(owner: string, name: string, personaIds: string[]) {
    const g = this.rows.get(this.at(owner, name));
    if (g) this.rows.set(this.at(owner, name), { ...g, personaIds });
    return Boolean(g);
  }
  async remove(owner: string, name: string) {
    return this.rows.delete(this.at(owner, name));
  }
  async dropMember() {}
}

/** Personas by id: key k owns p1-p3 and the default d; key other owns q1. */
const PERSONAS = {
  p1: { id: 'p1', owner: 'owner:k', isDefault: false, maxConcurrent: 1, lastUsedAt: '2026-01-03' },
  p2: { id: 'p2', owner: 'owner:k', isDefault: false, maxConcurrent: 2, lastUsedAt: '2026-01-01' },
  p3: { id: 'p3', owner: 'owner:k', isDefault: false, maxConcurrent: Infinity, lastUsedAt: null },
  d: { id: 'd', owner: 'owner:k', isDefault: true, maxConcurrent: Infinity, lastUsedAt: null },
  q1: { id: 'q1', owner: 'owner:other', isDefault: false, maxConcurrent: 1, lastUsedAt: null },
};

/** A service over a fresh repository, the persona lookup checking ownership as the real one does. */
function service(gone: Set<string> = new Set()) {
  const ownerOf = (key: string) => `owner:${key}`;
  const get = (key: string, id: string): any =>
    !gone.has(id) && PERSONAS[id]?.owner === ownerOf(key) ? PERSONAS[id] : null;
  return new GroupService({ repository: new MemoryGroups(), personas: { get }, ownerOf });
}

describe('GroupService', () => {
  it('makes a group of the caller’s personas, without repeats', async () => {
    const groups = service();
    const made = await groups.create('k', { name: 'pool', personaIds: ['p1', 'p2', 'p1'] });
    assert.deepEqual(made.personaIds, ['p1', 'p2']);
    assert.deepEqual(
      (await groups.list('k')).map((g) => g.name),
      ['pool'],
    );
    assert.deepEqual(await groups.list('other'), []);
  });

  it('refuses a name the caller already has, with group_exists', async () => {
    const groups = service();
    await groups.create('k', { name: 'pool', personaIds: [] });
    await assert.rejects(groups.create('k', { name: 'pool', personaIds: ['p1'] }), {
      status: 409,
      code: 'group_exists',
      message: 'A persona group named pool already exists',
    });
  });

  it('refuses another key’s persona and an unknown one with 404, and the default persona with 400', async () => {
    const groups = service();
    await assert.rejects(groups.create('k', { name: 'g', personaIds: ['q1'] }), {
      status: 404,
      code: 'not_found',
      message: 'No such persona: q1',
    });
    await assert.rejects(groups.create('k', { name: 'g', personaIds: ['p-x'] }), { status: 404 });
    await assert.rejects(groups.create('k', { name: 'g', personaIds: ['d'] }), {
      status: 400,
      message: 'The default persona cannot be in a group',
    });
  });

  it('refuses a bad name, a member list that is not ids, and a field it does not take', async () => {
    const groups = service();
    await assert.rejects(groups.create('k', { name: 'has space' }), { status: 400, code: 'invalid_request' });
    await assert.rejects(groups.create('k', { name: 'g', personaIds: 'p1' }), {
      status: 400,
      message: 'personaIds must be a list of persona ids',
    });
    await assert.rejects(groups.create('k', { name: 'g', owner: 'x' }), {
      status: 400,
      message: 'Unknown field: owner',
    });
    await assert.rejects(groups.create('k', null), { status: 400 });
  });

  it('replaces members and deletes, with 404 for a group the caller has not got', async () => {
    const groups = service();
    await groups.create('k', { name: 'pool', personaIds: ['p1'] });
    assert.deepEqual((await groups.replace('k', 'pool', { personaIds: ['p2'] })).personaIds, ['p2']);
    await assert.rejects(groups.replace('other', 'pool', { personaIds: [] }), {
      status: 404,
      message: 'No such persona group: pool',
    });
    await groups.remove('k', 'pool');
    await assert.rejects(groups.remove('k', 'pool'), { status: 404 });
  });

  it('offers a start its members used longest ago first, each with its cap, null for uncapped', async () => {
    const groups = service();
    await groups.create('k', { name: 'pool', personaIds: ['p1', 'p2', 'p3'] });
    assert.deepEqual(await groups.choices('k', 'pool'), [
      { persona: 'p3', personaLimit: null },
      { persona: 'p2', personaLimit: 2 },
      { persona: 'p1', personaLimit: 1 },
    ]);
  });

  it('leaves out members deleted since, and refuses a start on a group with none left', async () => {
    const gone = new Set<string>();
    const groups = service(gone);
    await groups.create('k', { name: 'pool', personaIds: ['p1', 'p2'] });
    gone.add('p1');
    assert.deepEqual((await groups.list('k'))[0].personaIds, ['p2']);
    gone.add('p2');
    await assert.rejects(groups.choices('k', 'pool'), { status: 400, code: 'invalid_request' });
    await assert.rejects(groups.choices('k', 'nope'), { status: 404 });
  });
});
