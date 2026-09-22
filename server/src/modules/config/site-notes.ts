/**
 * What the agent learned about a site and chose to keep for next time (where a
 * report lives, a login quirk), one sealed row per key and host in the same store
 * as playbooks. ponytail: in-memory per replica like the rest of this store.
 */
import { fingerprint as ownerOf } from '../../platform/audit.ts';
import { store, seal, unseal, writeField, dropField } from './store.ts';

/** The field every notes row starts with. */
const PREFIX = '_notes:';

/** The field a host's notes are stored under. */
const notesField = (host: string) => `${PREFIX}${host}`;

/** The notes kept for a host, oldest first; none when there are none or they no longer unseal. */
export function getSiteNotes(apiKey: string, host: string): string[] {
  const owner = ownerOf(apiKey);
  const sealed = store.get(owner)?.[notesField(host)];
  if (!sealed) return [];
  try {
    return unseal(owner, sealed);
  } catch {
    return [];
  }
}

/** Every host this key has notes for, with them, so the owner can read what the agent kept. */
export function allSiteNotes(apiKey: string): Record<string, string[]> {
  const owner = ownerOf(apiKey);
  const rows = Object.keys(store.get(owner) || {}).filter((field) => field.startsWith(PREFIX));
  return Object.fromEntries(
    rows.map((field) => [field.slice(PREFIX.length), getSiteNotes(apiKey, field.slice(PREFIX.length))]),
  );
}

/** Forgets a host's notes. */
export const forgetSiteNotes = (apiKey: string, host: string) => dropField(ownerOf(apiKey), notesField(host));

/** Keeps a host's notes, replacing what was there. */
export async function saveSiteNotes(apiKey: string, host: string, notes: string[]) {
  const owner = ownerOf(apiKey);
  await writeField(owner, notesField(host), seal(owner, notes));
}
