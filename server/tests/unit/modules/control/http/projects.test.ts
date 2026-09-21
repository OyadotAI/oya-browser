/**
 * Unit tests for the signed-in user's side of membership: listing the projects
 * they own or joined (and nobody else's), redeeming a one-time invitation, and
 * minting a console credential for a project they belong to.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-projects-');
const { control, projectId } = await import('../../../../../src/modules/control/service.ts');
const { consoleAccess, joinProject, listProjects } =
  await import('../../../../../src/modules/control/http/projects.ts');
const { inviteMember } = await import('../../../../../src/modules/control/http/members.ts');
const { patchRow } = await import('../../../support/control.ts');

let n = 0,
  owner,
  guest,
  A,
  B;
beforeEach(async () => {
  owner = `owner-${n}`;
  guest = `guest-${n}`;
  A = `key-a-${n}`;
  B = `key-b-${n++}`;
  for (const key of [A, B]) await control().project(key);
  await patchRow(control(), 'project', projectId(A), { ownerUser: owner, name: 'Acme' });
  await patchRow(control(), 'project', projectId(B), { ownerUser: 'someone-else' });
});

describe('joinProject', () => {
  it('makes the user a member with the invitation’s role', async () => {
    const { code } = await inviteMember(A, 'viewer');
    const m = await joinProject(guest, code);
    assert.deepEqual(m, { id: `${projectId(A)}:${guest}`, project: projectId(A), userId: guest, role: 'viewer' });
    assert.equal((await control().events(A)).at(-1).type, 'member.joined');
  });

  it('works only once', async () => {
    const { code } = await inviteMember(A, 'viewer');
    await joinProject(guest, code);
    await assert.rejects(joinProject('another', code), { status: 404, code: 'invalid_invite' });
  });

  it('refuses an expired or unknown code', async () => {
    const { code } = await inviteMember(A, 'viewer');
    await control().store.transact(async (tx) => {
      for (const row of await tx.list('invite', { project: projectId(A) })) row.expiresAt = 0;
    });
    await assert.rejects(joinProject(guest, code), { code: 'invalid_invite' });
    await assert.rejects(joinProject(guest, undefined), { code: 'invalid_invite' });
  });

  it('refuses an invitation to a project deleted since', async () => {
    const { code } = await inviteMember(A, 'viewer');
    await patchRow(control(), 'project', projectId(A), { deletedAt: 1 });
    await assert.rejects(joinProject(guest, code), { status: 404, code: 'not_found' });
  });
});

describe('listProjects', () => {
  it('lists owned projects as administrator and joined ones with their role', async () => {
    await joinProject(owner, (await inviteMember(B, 'operator')).code);
    assert.deepEqual(await listProjects(owner), [
      { id: projectId(A), name: 'Acme', role: 'administrator', owner: true },
      { id: projectId(B), name: (await control().project(B)).name, role: 'operator', owner: false },
    ]);
  });

  it('never lists projects the user neither owns nor joined', async () => {
    assert.deepEqual(await listProjects(guest), []);
  });

  it('leaves out deleted projects', async () => {
    await joinProject(guest, (await inviteMember(B, 'viewer')).code);
    await patchRow(control(), 'project', projectId(B), { deletedAt: 1 });
    assert.deepEqual(await listProjects(guest), []);
  });
});

describe('consoleAccess', () => {
  it('mints a one-hour administrator credential for the owner', async () => {
    const c = await consoleAccess(owner, projectId(A));
    assert.equal(c.role, 'administrator');
    assert.equal(c.label, 'Console access');
    assert.equal((await control().authenticate(c.token)).key, A);
  });

  it('mints a member’s credential with their role', async () => {
    await joinProject(guest, (await inviteMember(A, 'viewer')).code);
    assert.equal((await consoleAccess(guest, projectId(A))).role, 'viewer');
  });

  it('answers 404 to a stranger, and for a deleted project', async () => {
    await assert.rejects(consoleAccess(guest, projectId(A)), { status: 404 });
    await patchRow(control(), 'project', projectId(A), { deletedAt: 1 });
    await assert.rejects(consoleAccess(owner, projectId(A)), { status: 404 });
  });
});
