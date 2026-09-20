/** The project side of membership: inviting members and removing them. */
import { randomBytes } from 'node:crypto';
import { control, hash, fault, projectId } from '../service.ts';
import { Status } from '../../../platform/http-status.ts';
import { INVITE_CODE_BYTES, INVITE_TTL_S, MS_PER_SECOND } from './constants.ts';

/** Roles a member may hold. */
const ROLES = ['viewer', 'operator', 'administrator'];

/** A one-time invitation code for the key's project, valid for seven days. */
export async function inviteMember(key, role) {
  if (!ROLES.includes(role)) throw fault('invalid_role', 'Unknown role', Status.BAD_REQUEST);
  const code = randomBytes(INVITE_CODE_BYTES).toString('base64url');
  await control().store.transact(async (tx) => storeInvite(tx, key, code, role));
  return { code, expiresIn: INVITE_TTL_S };
}

/** Stores the invite under the code's hash and records it in the audit log. */
function storeInvite(tx, key, code, role) {
  tx.put('invite', hash(code), { project: projectId(key), role, expiresAt: Date.now() + INVITE_TTL_S * MS_PER_SECOND });
  tx.emit(projectId(key), 'member.invited', null, { role });
}

/** Remove a member from the key's project and revoke their credentials; the owner cannot be removed. */
export async function removeMember(key, userId) {
  await control().store.transact((tx) => dropMember(tx, key, userId));
  return { ok: true };
}

/** The removal transaction. */
async function dropMember(tx, key, userId) {
  const project = projectId(key),
    id = `${project}:${userId}`;
  if ((await tx.get('project', project))?.ownerUser === userId)
    throw fault('owner_required', 'The project owner cannot be removed');
  if (await tx.get('membership', id)) await tx.delete('membership', id);
  for (const c of await tx.list('credential', { project }))
    if (c.memberUser === userId && !c.revokedAt) c.revokedAt = Date.now();
  tx.emit(project, 'member.removed', null, { userId });
}
