/**
 * The signed-in user's side of project membership: listing their projects,
 * redeeming an invitation, and minting console access.
 */
import { listApiKeys } from '../../auth/service.ts';
import { control, hash, fault } from '../service.ts';
import { Status } from '../../../platform/http-status.ts';
import { CONSOLE_ACCESS_MS } from './constants.ts';

/** The name a project gets before anyone names it. */
export const AUTO_NAME = /^Project [0-9a-f]{6}$/;

/** Projects the user owns or has joined, with their role in each. */
export async function listProjects(userId) {
  const { owned, memberships } = await ownedAndJoined(userId);
  // A project you own is named after its API key's label, so the switcher and the project overview agree.
  const labels = new Map(
    (await listApiKeys(userId)).filter((k) => k.label && k.project).map((k) => [k.project, k.label]),
  );
  await nameAfterLabels(owned, labels);
  const names = await projectNames(memberships);
  return [...owned.map((r) => ownedEntry(r, labels)), ...joinedEntries(memberships, names)];
}

/** The user's live owned projects, and memberships of projects they do not own. */
async function ownedAndJoined(userId) {
  // Projects index their owner and memberships index their user, so this never scans other tenants.
  const [ownedRows, joined] = await control().store.load([
    { kind: 'project', states: [userId] },
    { kind: 'membership', states: [userId] },
  ]);
  const owned = ownedRows.filter((r) => !r.body.deletedAt);
  const ownedIds = new Set(owned.map((r) => r.id));
  return { owned, memberships: joined.map((r) => r.body).filter((m) => !ownedIds.has(m.project)) };
}

/** Renames still auto-named owned projects after their key's label. */
async function nameAfterLabels(owned, labels) {
  const unnamed = owned.filter((r) => labels.has(r.id) && AUTO_NAME.test(r.body.name)).map((r) => r.id);
  if (!unnamed.length) return;
  await control().store.transact(async (tx) => {
    for (const p of await tx.getMany('project', unnamed)) if (AUTO_NAME.test(p.name)) p.name = labels.get(p.id);
  });
}

/** Names of the live projects behind these memberships. */
async function projectNames(memberships) {
  const rows = await control().store.load(memberships.map((m) => ({ kind: 'project', id: m.project })));
  return new Map(
    rows
      .flat()
      .filter((r) => !r.body.deletedAt)
      .map((r) => [r.id, r.body.name]),
  );
}

/** An owned project as the list shows it. */
const ownedEntry = (r, labels) => ({
  id: r.id,
  name: AUTO_NAME.test(r.body.name) ? labels.get(r.id) || r.body.name : r.body.name,
  role: 'administrator',
  owner: true,
});

/** Joined projects that still exist, as the list shows them. */
const joinedEntries = (memberships, names) =>
  memberships
    .filter((m) => names.has(m.project))
    .map((m) => ({ id: m.project, name: names.get(m.project), role: m.role, owner: false }));

/** Redeems a one-time invitation code; the user becomes a member with its role. */
export async function joinProject(userId, code) {
  const digest = hash(String(code || ''));
  return control().store.transact((tx) => redeem(tx, digest, userId));
}

/** The redemption transaction: consume the invite and write the membership. */
async function redeem(tx, digest, userId) {
  const invite = await validInvite(tx, digest);
  await tx.delete('invite', digest);
  const id = `${invite.project}:${userId}`;
  await tx.get('membership', id);
  const membership = tx.put('membership', id, { id, project: invite.project, userId, role: invite.role });
  tx.emit(invite.project, 'member.joined', null, { userId, role: invite.role });
  return membership;
}

/** The unexpired invite for this digest, whose project still exists. */
async function validInvite(tx, digest) {
  const invite = await tx.get('invite', digest);
  if (!invite || invite.expiresAt < Date.now())
    throw fault('invalid_invite', 'Invitation expired or already used', Status.NOT_FOUND);
  if ((await tx.get('project', invite.project))?.deletedAt)
    throw fault('not_found', 'Project not found', Status.NOT_FOUND);
  return invite;
}

/** A one-hour console credential for a project the user owns or belongs to. */
export async function consoleAccess(userId, id) {
  const [[p], [m]] = await control().store.load([
    { kind: 'project', id },
    { kind: 'membership', id: `${id}:${userId}` },
  ]);
  const role = p?.body.ownerUser === userId ? 'administrator' : m?.body.role;
  if (!p || p.body.deletedAt || !role) throw fault('not_found', 'Project not found', Status.NOT_FOUND);
  const expiresAt = Date.now() + CONSOLE_ACCESS_MS;
  return control().credential(control().projectKey(p.body), { role, label: 'Console access', expiresAt }, userId);
}
