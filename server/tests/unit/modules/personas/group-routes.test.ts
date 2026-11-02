/**
 * Unit tests for the persona group routes (personas/group-routes.ts): a key
 * makes, lists, changes and deletes its own groups, gets 404 for another key's,
 * and the service's refusals answer with their status and code.
 */
import { describe, it, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-group-routes-');
const { personaGroupRoutes } = await import('../../../../src/modules/personas/group-routes.ts');
const { GroupService, GroupTable } = await import('../../../../src/modules/personas/index.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const KEY = 'group-routes-key';
const OTHER = 'group-routes-other';
/** The key's one persona. */
const MINE = { id: 'p-mine', owner: fingerprint(KEY), isDefault: false, maxConcurrent: 1, lastUsedAt: null } as any;

const groups = new GroupService({
  repository: new GroupTable(),
  personas: { get: (key: string, id: string) => (id === MINE.id && fingerprint(key) === MINE.owner ? MINE : null) },
  ownerOf: fingerprint,
});
const router = personaGroupRoutes(groups);

/** Calls the router as `key`. */
const call = (method: string, url: string, body?: unknown, key = KEY) => callRoute(router, { method, url, body, key });

describe('persona group routes', () => {
  let forget: (() => void)[] = [];
  before(() => (forget = [allowKey(KEY), allowKey(OTHER)]));
  after(() => forget.forEach((f) => f()));
  beforeEach(() => getConnection().delete('persona_groups', {}));

  it('makes, lists, changes and deletes a group', async () => {
    const made = await call('POST', '/persona-groups', { name: 'pool', personaIds: [MINE.id] });
    assert.equal(made.status, 201);
    assert.deepEqual(made.body.personaIds, [MINE.id]);
    assert.deepEqual(
      (await call('GET', '/persona-groups')).body.groups.map((g) => g.name),
      ['pool'],
    );
    assert.deepEqual((await call('PUT', '/persona-groups/pool', { personaIds: [] })).body.personaIds, []);
    assert.deepEqual((await call('DELETE', '/persona-groups/pool')).body, { ok: true });
  });

  it('answers a refusal with its status and code, and 404 for another key’s group', async () => {
    await call('POST', '/persona-groups', { name: 'pool', personaIds: [] });
    const again = await call('POST', '/persona-groups', { name: 'pool', personaIds: [] });
    assert.deepEqual([again.status, again.body.code], [409, 'group_exists']);
    assert.equal((await call('PUT', '/persona-groups/pool', { personaIds: [] }, OTHER)).status, 404);
    assert.equal((await call('DELETE', '/persona-groups/pool', undefined, OTHER)).status, 404);
    assert.deepEqual((await call('GET', '/persona-groups', undefined, OTHER)).body.groups, []);
  });

  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(router, { url: '/persona-groups' })).status, 401);
  });
});
