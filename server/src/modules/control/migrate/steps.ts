/**
 * The steps of the legacy ownership migration: back up the data directory, give
 * each project its owning user, and import legacy browser rows as sessions.
 * Each step is safe to re-run.
 */
import { mkdir, readdir, copyFile, cp } from 'node:fs/promises';
import { join } from 'node:path';
import { knownKeys, getKeyOwner } from '../../auth/service.ts';
import { db } from '../../../platform/db.ts';
import { BACKUP_DIR_MODE } from './constants.ts';

/** Entries of the data directory the backup leaves out: the control database, earlier backups, recordings. */
const skipped = (name) => name.startsWith('control.sqlite') || name.startsWith('migration-') || name === 'recordings';

/** Copy one entry into the backup, never overwriting what an earlier run copied. */
async function backupEntry(entry, directory, backup) {
  const source = join(directory, entry.name),
    destination = join(backup, entry.name);
  if (entry.isDirectory()) await cp(source, destination, { recursive: true, force: false });
  else if (entry.isFile())
    await copyFile(source, destination, 1).catch((e) => {
      if (e.code !== 'EEXIST') throw e;
    });
}

/** Copy the data directory into a private `backup` directory. */
export async function backupData(directory, backup) {
  await mkdir(backup, { recursive: true, mode: BACKUP_DIR_MODE });
  for (const entry of await readdir(directory, { withFileTypes: true }))
    if (!skipped(entry.name)) await backupEntry(entry, directory, backup);
}

/** Give one key's project the key's owner; returns the mapping entry. */
async function ownKey(service, key) {
  const project = await service.project(key),
    owner = await getKeyOwner(key);
  if (owner)
    await service.store.transact(async (tx) => {
      (await tx.get('project', project.id)).ownerUser = owner;
    });
  return { project: project.id, legacyOwner: project.legacyOwner };
}

/** Give a database key's project its user, unless it already has an owner; returns the mapping entry. */
async function ownProject(service, row) {
  await service.store.transact(async (tx) => {
    const p = await tx.get('project', row.project);
    if (p && !p.ownerUser) p.ownerUser = row.user_id;
  });
  return { project: row.project };
}

/** Give every project this server can find its owning user; returns the mapping written beside the backup. */
export async function assignOwners(service) {
  const mapping = [];
  // Env keys and the fleet token are the only ones this process holds in the
  // clear; a database key is reached through the project id stored beside its
  // digest, because api_keys no longer keeps the key itself.
  for (const key of knownKeys()) mapping.push(await ownKey(service, key));
  if (!db) return mapping;
  const { data } = await db.from('api_keys').select('project, user_id');
  for (const row of data || []) if (row.project && row.user_id) mapping.push(await ownProject(service, row));
  return mapping;
}

/** A legacy browser row as a disconnected session awaiting reconnect or inspection. */
function legacySession(row, project) {
  return {
    ...{ id: row.id, project: project.id, provider: 'legacy', name: row.name, state: 'disconnected', managed: false },
    ...{ createdAt: Date.now(), updatedAt: Date.now(), costUsd: 0, control: { mode: 'agent' }, fence: 0 },
    leaseUntil: 0,
    recoveryNote: 'Imported inventory; reconnect or inspect the provider before cleanup',
  };
}

/** Import one legacy browser row as a session, unless it already is one. */
async function importBrowser(service, row) {
  const project = await service.project(row.api_key);
  await service.store.transact(async (tx) => {
    if (!(await tx.get('session', row.id))) tx.put('session', row.id, legacySession(row, project));
  });
}

/** Import the legacy browser inventory, when there is a database; throws, leaving the migration unmarked, when it cannot be read. */
export async function importBrowsers(service) {
  if (!db) return;
  const { data, error } = await db.from('browsers').select('id, api_key, name');
  if (error) throw new Error('Could not read legacy browser inventory; migration was not marked complete');
  for (const row of data || []) if (row.api_key && row.id) await importBrowser(service, row);
}
