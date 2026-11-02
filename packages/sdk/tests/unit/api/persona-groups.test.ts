/**
 * Unit tests for `oya.personaGroups` (src/api/persona-groups.ts): each call's
 * method, path and body, the name encoded in the path, and the alias.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { client } from '../support/fake-fetch.ts';

const GROUP = { name: 'pool', personaIds: ['p1'], createdAt: 'c', updatedAt: 'u' };

describe('oya.personaGroups', () => {
  it('lists, creates, updates and removes groups', async () => {
    const { oya, calls } = client({
      'GET /api/persona-groups': { body: { groups: [GROUP] } },
      'POST /api/persona-groups': { body: GROUP },
      'PUT /api/persona-groups/pool': { body: { ...GROUP, personaIds: [] } },
      'DELETE /api/persona-groups/pool': { body: { ok: true } },
    });
    assert.deepEqual(await oya.personaGroups.list(), [GROUP]);
    assert.deepEqual(await oya.personaGroups.create('pool', ['p1']), GROUP);
    assert.deepEqual((await oya.personaGroups.update('pool', [])).personaIds, []);
    assert.equal(await oya.personaGroups.remove('pool'), undefined);
    assert.deepEqual(
      calls.map((c) => [c.method, c.path, c.body]),
      [
        ['GET', '/api/persona-groups', undefined],
        ['POST', '/api/persona-groups', { name: 'pool', personaIds: ['p1'] }],
        ['PUT', '/api/persona-groups/pool', { personaIds: [] }],
        ['DELETE', '/api/persona-groups/pool', undefined],
      ],
    );
  });

  it('creates a group with no members when none are given, and answers to profileGroups too', async () => {
    const { oya, calls } = client({ 'POST /api/persona-groups': { body: GROUP } });
    await oya.profileGroups.create('pool');
    assert.deepEqual(calls[0].body, { name: 'pool', personaIds: [] });
  });

  it('refuses a name that would change the path, before any request', async () => {
    const { oya, calls } = client({});
    await assert.rejects(oya.personaGroups.remove('../keys'));
    assert.equal(calls.length, 0);
  });
});
