/**
 * Playbooks ride the same sealed store as routing, one row each; their steps can
 * hold whatever the user typed. ponytail: in-memory per replica like the rest of
 * this store; move to a table if playbooks must appear on other replicas without a restart.
 */
import { fingerprint as ownerOf } from '../../platform/audit.ts';
import { cleanPlaybook } from '../playbooks/cleanup.ts';
import { store, seal, unseal, flush, writeField, dropField, markChanged } from './store.ts';

/** The field every playbook row name starts with. */
const PREFIX = '_playbook:';
/** The field a playbook is stored under. */
const playbookField = (name) => `${PREFIX}${name}`;

/** Store a playbook under `name`, sealed, replacing any with that name. */
export async function savePlaybook(apiKey, name, playbook) {
  const owner = ownerOf(apiKey);
  await writeField(owner, playbookField(name), seal(owner, playbook));
}

/** Remove a stored playbook. */
export async function deletePlaybook(apiKey, name) {
  await dropField(ownerOf(apiKey), playbookField(name));
}

/** Every playbook and draft this key saved, named by the field they are stored under. */
export function listPlaybooks(apiKey) {
  const owner = ownerOf(apiKey);
  return Object.entries(store.get(owner) || {})
    .filter(([field]) => field.startsWith(PREFIX))
    .map(([field]) => getPlaybook(apiKey, field.slice(PREFIX.length)))
    .filter(Boolean);
}

/** One stored playbook with its name, or null. */
export function getPlaybook(apiKey, name) {
  const owner = ownerOf(apiKey);
  const sealed = store.get(owner)?.[playbookField(name)];
  if (!sealed) return null;
  // Like reveal(): a row sealed under a rotated secret reads as absent instead of failing every listing.
  try {
    return { ...unseal(owner, sealed), name };
  } catch {
    return null;
  }
}

/** A sealed playbook opened, or null when it no longer unseals. */
function open(owner, sealed) {
  try {
    return { value: unseal(owner, sealed) };
  } catch {
    return null;
  }
}

/** Swap in the cleaned playbook, keeping the first sealed original as a backup. */
function replace(owner, row, field, sealed, cleaned) {
  const replacement = seal(owner, cleaned);
  const backup = `_playbook-backup:v1:${field.slice(PREFIX.length)}`;
  if (!Object.hasOwn(row, backup)) row[backup] = sealed;
  row[field] = replacement;
  markChanged(owner);
}

/** Clean one sealed playbook in place: 'updated', 'unreadable', or null when already clean. */
function cleanOne(owner, row, field, sealed) {
  const opened = open(owner, sealed);
  if (!opened) return 'unreadable';
  const cleaned = cleanPlaybook(opened.value);
  if (JSON.stringify(cleaned) === JSON.stringify(opened.value)) return null;
  replace(owner, row, field, sealed, cleaned);
  return 'updated';
}

/** Clean every playbook in one owner's row, counting what happened. */
function cleanRow(counts, owner, row) {
  for (const [field, sealed] of Object.entries(row)) {
    const outcome = field.startsWith(PREFIX) && cleanOne(owner, row, field, sealed);
    if (outcome) counts[outcome]++;
  }
}

/** Upgrade saved playbooks and healed drafts without losing their sealed originals. */
export async function cleanupPlaybooks() {
  const counts = { updated: 0, unreadable: 0 };
  for (const [owner, row] of store) cleanRow(counts, owner, row);
  if (counts.updated) await flush();
  return counts;
}
