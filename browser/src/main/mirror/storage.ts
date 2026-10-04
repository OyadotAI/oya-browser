/**
 * Seeding one persona's Electron partition with a real profile's site storage
 * (localStorage, IndexedDB, service workers). Cookies reach the pool over the
 * socket, but these are files, so they are copied straight into the partition
 * before its tabs open. Only the desktop's own persona is seeded; a remote
 * browser cannot read local files, and gets its logins from the cookie pool.
 */
import path from 'node:path';
import type { AppServices } from '../app/services.ts';
import { copyIfPresent } from './copy.ts';
import { PROFILE_STORES } from './constants.ts';

/** The part of Electron seeding needs: where the app keeps its data. */
export type StorageElectron = Pick<AppServices['electron'], 'app'>;

/** The on-disk directory Electron keeps a persistent partition's data in. */
function partitionDir(electron: StorageElectron, personaId: string): string {
  return path.join(electron.app.getPath('userData'), 'Partitions', `oya-${personaId}`);
}

/** Copies a real profile's storage directories into the persona's partition. */
export function seedStorage(
  electron: StorageElectron,
  personaId: string,
  userDataDir: string,
  profileDir: string,
): void {
  const dest = partitionDir(electron, personaId);
  const from = path.join(userDataDir, profileDir);
  for (const store of PROFILE_STORES) copyIfPresent(path.join(from, store), path.join(dest, store));
}
