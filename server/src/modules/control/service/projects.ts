/**
 * Projects: created on a key's first use, read as an overview, renamed,
 * reconfigured and deleted by their owner.
 */
import { sealText, openText } from '../../../platform/secrets.ts';
import { Status } from '../../../platform/http-status.ts';
import { fault, hash, projectDeleted, projectId, stamp, terminal } from './model.ts';
import { publicSession, stopSession } from './sessions.ts';
import { validateSettings } from './policy.ts';
import {
  DEFAULT_AUDIT_DAYS,
  DEFAULT_RECORDING_DAYS,
  LEGACY_OWNER_CHARS,
  MAX_PROJECT_NAME,
  OVERVIEW_EVENTS,
  PROJECT_NAME_SUFFIX,
} from './constants.ts';

/** A project without its sealed key and internal bookkeeping. */
export const publicProject = ({ key, alerts, recentCloud, ...rest }) => rest;

/** What unsealing fails with when this server's secret cannot open a project's key. */
const KEY_UNAVAILABLE =
  'Project credentials could not be decrypted. Restore the original OYA_PROFILE_SECRET and OYA_PROFILE_SALT on every server, or add the original API key again to repair project access.';

/** A new project's settings: default retention, no limits, no rates, no policy. */
const defaultSettings = () => ({
  recordingDays: DEFAULT_RECORDING_DAYS,
  auditDays: DEFAULT_AUDIT_DAYS,
  budgetUsd: null,
  maxConcurrent: null,
  rates: {},
  policy: {},
});

/** A new project row for `key`, which it keeps sealed. */
function newProject(id, key) {
  const name = `Project ${id.slice(-PROJECT_NAME_SUFFIX)}`,
    createdAt = stamp(),
    legacyOwner = hash(key).slice(0, LEGACY_OWNER_CHARS),
    sealed = sealText(`control:${id}`, key);
  return { id, name, createdAt, legacyOwner, key: sealed, costUsd: 0, settings: defaultSettings() };
}

/** The key's project, created on first use; refuses a deleted one. */
export async function ensure(tx, key) {
  const id = projectId(key);
  let p = await tx.get('project', id);
  if (!p) {
    p = tx.put('project', id, newProject(id, key));
    tx.emit(id, 'project.created');
  }
  if (p.deletedAt) throw projectDeleted();
  return p;
}

/** The key's project, created on first use, without its sealed key. */
export async function projectOf(store, key) {
  const p = await store.get('project', projectId(key));
  if (p?.deletedAt) throw projectDeleted();
  return publicProject(p || (await store.transact((tx) => ensure(tx, key))));
}

/** Unseal the API key a project row holds; 503 when this server's secret cannot open it. */
export function openProjectKey(project) {
  try {
    return openText(`control:${project.id}`, project.key);
  } catch {
    throw fault('project_key_unavailable', KEY_UNAVAILABLE, Status.UNAVAILABLE);
  }
}

/** Merge validated changes into the project's settings. */
export async function updateSettings(store, key, changes) {
  validateSettings(changes);
  return store.transact(async (tx) => {
    const p = await ensure(tx, key);
    p.settings = { ...p.settings, ...changes };
    tx.emit(p.id, 'project.settings.updated', null, { fields: Object.keys(changes) });
    return publicProject(p);
  });
}

/** Refuse to delete a project with open browsers unless the owner asked to stop them. */
function refuseOpen(open, stopBrowsers) {
  if (!open.length || stopBrowsers) return;
  const message = `Stop ${open.length} browser${open.length === 1 ? '' : 's'} before deleting this project`;
  throw Object.assign(fault('project_active', message), { active: open.length });
}

/** Revoke a deleted project's credentials and remove its memberships. */
async function revokeAccess(tx, id) {
  for (const c of await tx.list('credential', { project: id })) c.revokedAt = stamp();
  for (const m of await tx.list('membership', { project: id })) await tx.delete('membership', m.id);
}

/** Delete a project: stop its browsers (when allowed), mark it deleted and revoke its access. */
async function deleteProject(tx, p, id, stopBrowsers) {
  const open = (await tx.list('session', { project: id })).filter((s) => !terminal.has(s.state));
  refuseOpen(open, stopBrowsers);
  // Deleting is the owner's final word, so a session nothing can reach (disconnected, unknown outcome) ends here.
  // Resources with a deletion descriptor keep cleaning up after the project is gone.
  for (const x of open) stopSession(tx, x, { force: true, reason: 'project_deleted' });
  p.deletedAt = stamp();
  await revokeAccess(tx, id);
  tx.emit(id, 'project.deleted');
}

/** Rename a project; names are 1–100 characters once trimmed. */
function renameProject(tx, p, id, name) {
  if (typeof name !== 'string' || !name.trim() || name.trim().length > MAX_PROJECT_NAME)
    throw fault('invalid_name', 'Project name must be 1–100 characters', Status.BAD_REQUEST);
  p.name = name.trim();
  tx.emit(id, 'project.renamed');
}

/**
 * Rename or delete a project its signed-in owner holds. Deleting refuses while
 * browsers are open unless `stopBrowsers`, and revokes its credentials and memberships.
 */
export async function updateOwnedProject(store, userId, id, { name, remove = false, stopBrowsers = false }: any = {}) {
  return store.transact(async (tx) => {
    const p = await tx.get('project', id);
    if (!p || p.ownerUser !== userId || p.deletedAt) throw fault('not_found', 'Project not found', Status.NOT_FOUND);
    if (remove) await deleteProject(tx, p, id, stopBrowsers);
    else renameProject(tx, p, id, name);
    return { ok: true };
  });
}

/** What the overview reads in one round trip. */
const overviewQueries = (id) => [
  { kind: 'session', project: id },
  { kind: 'credential', project: id },
  { kind: 'webhook', project: id },
  // Delivered notifications need no attention; the overview lists the rest.
  { kind: 'delivery', project: id, states: ['pending', 'failed', 'cancelled'] },
  { kind: 'meta', id: 'draining' },
];

/** The overview's lists, secrets stripped: people's credentials, webhooks without secrets, open deliveries. */
function listings(credentials, webhooks, deliveries) {
  return {
    credentials: credentials
      .map((r) => r.body)
      .filter((c) => c.role !== 'browser')
      .map(({ digest, ...c }) => c),
    webhooks: webhooks.map(({ body: { secret, ...hook } }) => hook),
    deliveries: deliveries.map((r) => r.body),
  };
}

/** The project overview: sessions, recent events, credentials, webhooks and undelivered deliveries, secrets stripped. */
export async function overview(service, key) {
  const project = await service.project(key);
  const [sessions, credentials, webhooks, deliveries, [draining]] = await service.store.load(
    overviewQueries(project.id),
  );
  const head = { project, draining: !!draining?.body.value, sessions: sessions.map((r) => publicSession(r.body)) };
  const events = await service.store.events({ project: project.id, latest: true, limit: OVERVIEW_EVENTS });
  return { ...head, events, ...listings(credentials, webhooks, deliveries) };
}
