/**
 * Unit tests for the project side of membership: invitations stored by their
 * code's hash for seven days, and removing a member, which revokes their
 * credentials in this project only and never removes the owner.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-members-');
const { control, hash, projectId } = await import('../../../../../src/modules/control/service.ts');
const { inviteMember, removeMember } = await import('../../../../../src/modules/control/membership.ts');
const { patchRow, putRow } = await import('../../../support/control.ts');

const NOW = 1_700_000_000_000;
let n = 0,
  A,
  B;
beforeEach(async () => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  A = `key-a-${n}`;
  B = `key-b-${n++}`;
  await control().project(A);
  await patchRow(control(), 'project', projectId(A), { ownerUser: 'owner' });
});
afterEach(() => mock.timers.reset());

/** Makes `user` a member of the key's project. */
const join = (key, user, role = 'operator') =>
  putRow(control(), 'membership', `${projectId(key)}:${user}`, {
    id: `${projectId(key)}:${user}`,
    project: projectId(key),
    userId: user,
    role,
  });

describe('inviteMember', () => {
  it('stores the invite under its code’s hash for seven days and audits it', async () => {
    const { code, expiresIn } = await inviteMember(A, 'viewer');
    assert.equal(expiresIn, 604_800);
    assert.deepEqual(await control().store.get('invite', hash(code)), {
      project: projectId(A),
      role: 'viewer',
      expiresAt: NOW + 604_800_000,
    });
    const last = (await control().events(A)).at(-1);
    assert.deepEqual([last.type, last.detail], ['member.invited', { role: 'viewer' }]);
  });

  it('refuses an unknown role', async () => {
    await assert.rejects(inviteMember(A, 'owner'), { status: 400, code: 'invalid_role' });
  });
});

describe('removeMember', () => {
  it('removes the membership and revokes that member’s credentials here', async () => {
    await join(A, 'u-2');
    const c = await control().credential(A, { role: 'operator' }, 'u-2');
    assert.deepEqual(await removeMember(A, 'u-2'), { ok: true });
    assert.equal(await control().store.get('membership', `${projectId(A)}:u-2`), null);
    assert.ok((await control().store.get('credential', hash(c.token))).revokedAt);
    await assert.rejects(control().authenticate(c.token), { status: 403 });
    assert.deepEqual((await control().events(A)).at(-1).detail, { userId: 'u-2' });
  });

  it('answers 404 for a member that is not in the project, instead of a quiet ok', async () => {
    await assert.rejects(removeMember(A, 'nope'), { status: 404, code: 'not_found', message: 'Member not found' });
  });

  it('leaves the member’s access to other projects alone', async () => {
    await join(A, 'u-2');
    await join(B, 'u-2');
    const theirs = await control().credential(B, { role: 'operator' }, 'u-2');
    await removeMember(A, 'u-2');
    assert.equal((await control().authenticate(theirs.token)).key, B);
  });

  it('refuses to remove the owner', async () => {
    await assert.rejects(removeMember(A, 'owner'), { status: 409, code: 'owner_required' });
  });
});
