/**
 * Project credentials: issued to people, services, managed browsers and share
 * links; resolved to a principal on every request; revoked.
 */
import { randomBytes, randomUUID } from 'node:crypto';
import { Status } from '../../../platform/http-status.ts';
import { fault, hash, projectDeleted, projectId, stamp, terminal } from './model.ts';
import { ensure } from './projects.ts';
import {
  DEFAULT_SHARE_SECONDS,
  MAX_LABEL,
  MAX_SHARE_SECONDS,
  MIN_SHARE_SECONDS,
  MS_PER_SECOND,
  TOKEN_BYTES,
} from './constants.ts';

/** Roles a credential may be issued with. */
const ROLES = ['viewer', 'operator', 'administrator'];

/** The role a user holds in a project: the owner administers it; others hold their membership's role. */
async function roleOf(tx, p, userId) {
  return p.ownerUser === userId ? 'administrator' : (await tx.get('membership', `${p.id}:${userId}`))?.role;
}

/** A stored credential: only the token's digest is kept. */
function credentialRow(id, digest, p, { role, label, expiresAt, memberUser, sessionId = null }) {
  const who = { id, digest, project: p.id, role, memberUser, sessionId };
  return { ...who, label: String(label).slice(0, MAX_LABEL), expiresAt, createdAt: stamp(), revokedAt: null };
}

/** The issuing transaction: the member must still hold the role; returns the credential without its digest. */
async function storeCredential(tx, key, grant, digest, id) {
  const p = await ensure(tx, key);
  if (grant.memberUser && (await roleOf(tx, p, grant.memberUser)) !== grant.role)
    throw fault('membership_changed', 'Project membership changed', Status.FORBIDDEN);
  const { digest: _, ...publicValue } = tx.put('credential', digest, credentialRow(id, digest, p, grant));
  tx.emit(p.id, 'credential.created', null, { id, role: grant.role });
  return publicValue;
}

/** Mint and store a credential; only its digest is kept, and the token is returned once. */
export async function issue(controlStore, key, grant) {
  const token = `oya_${randomBytes(TOKEN_BYTES).toString('base64url')}`,
    digest = hash(token),
    id = randomUUID();
  const credential = await controlStore.transact((tx) => storeCredential(tx, key, grant, digest, id));
  return { ...credential, token };
}

/** Issue a project credential for a person or service. `memberUser` must still hold `role` in the project. */
export async function credential(controlStore, key, options: any = {}, memberUser = null) {
  const { role = 'operator', label = 'Service account', expiresAt = null } = options;
  if (!ROLES.includes(role)) throw fault('invalid_role', 'Unknown role', Status.BAD_REQUEST);
  if (expiresAt !== null && (!Number.isFinite(expiresAt) || expiresAt <= stamp()))
    throw fault('invalid_expiry', 'Expiry must be a future timestamp', Status.BAD_REQUEST);
  return issue(controlStore, key, { role, label, expiresAt, memberUser });
}

/** A share link's lifetime in seconds: the caller's, whole, clamped to one minute–30 days. */
const shareSeconds = (expiresIn) =>
  Math.min(Math.max(Math.floor(Number(expiresIn) || DEFAULT_SHARE_SECONDS), MIN_SHARE_SECONDS), MAX_SHARE_SECONDS);

/**
 * A shareable, expiring credential for one live browser — embed it in a link
 * and hand it to a viewer, or to another app via the SDK. `control` decides
 * whether the holder can only watch (viewer) or also take over and act
 * (operator). The credential is bound to this one session and confined to its
 * live-view, ticket, input and control endpoints by authMiddleware, so a share
 * link never becomes project-wide access. Revoke it like any credential.
 */
export async function share(service, key, { id, control = false, expiresIn = DEFAULT_SHARE_SECONDS }: any = {}) {
  const x = await service.findSession(key, id);
  if (!x || terminal.has(x.state)) throw fault('not_found', 'Ready session not found', Status.NOT_FOUND);
  const expiresAt = stamp() + shareSeconds(expiresIn) * MS_PER_SECOND;
  const label = control ? 'Shared browser (control)' : 'Shared browser (view)';
  const grant = { role: control ? 'operator' : 'viewer', label, expiresAt, memberUser: null, sessionId: id };
  return issue(service.store, key, grant);
}

/** Refuse a credential whose project is gone or whose member lost the role. */
function assertEntitled(c, project, membership) {
  if (!project || project.body.deletedAt) throw projectDeleted();
  if (c.memberUser && (project.body.ownerUser === c.memberUser ? 'administrator' : membership?.body.role) !== c.role)
    throw fault('membership_removed', 'Project access was removed', Status.FORBIDDEN);
}

/** Refuse a revoked or expired credential, and a managed browser's once its session ended. */
function assertCurrent(c, session) {
  if (c.revokedAt || (c.expiresAt && c.expiresAt <= stamp()))
    throw fault('revoked_credential', 'Credential expired or revoked', Status.UNAUTHORIZED);
  if (c.role === 'browser' && (!session || terminal.has(session.body.state)))
    throw fault('revoked_credential', 'Managed browser session has ended', Status.UNAUTHORIZED);
}

/** What a credential's checks read in one round trip: its project, its member's membership, its session. */
const principalQueries = (c) => [
  { kind: 'project', id: c.project },
  { kind: 'membership', id: `${c.project}:${c.memberUser}` },
  { kind: 'session', id: String(c.sessionId) },
];

/** Resolve a credential token to its principal; null if unknown, throws if revoked, expired or no longer entitled. */
export async function authenticate(service, token) {
  const c = await service.store.get('credential', hash(token));
  if (!c) return null;
  const [[project], [membership], [session]] = await service.store.load(principalQueries(c));
  assertEntitled(c, project, membership);
  assertCurrent(c, session);
  const key = service.projectKey(project.body);
  return { key, role: c.role, project: c.project, credentialId: c.id, sessionId: c.sessionId };
}

/** Revoke one of the project's credentials. */
export async function revoke(controlStore, key, id) {
  return controlStore.transact(async (tx) => {
    const c = (await tx.list('credential', { project: projectId(key) })).find((c) => c.id === id);
    if (!c) throw fault('not_found', 'Credential not found', Status.NOT_FOUND);
    c.revokedAt = stamp();
    tx.emit(c.project, 'credential.revoked', null, { id });
    return { ok: true };
  });
}
