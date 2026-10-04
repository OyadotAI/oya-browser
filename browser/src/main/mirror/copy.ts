/**
 * Copying profile files safely: skip a source's lock files, and copy only what
 * is there. Shared by the capture snapshot and the partition seeding.
 */
import fs from 'node:fs';

/** A LevelDB/lock file, skipped so a copy never carries the source's lock. */
export const isLock = (src: string): boolean => /(^|[\\/])(LOCK|lockfile)$/i.test(src);

/** Copies one file or directory tree when it exists, leaving lock files behind. */
export function copyIfPresent(from: string, to: string): void {
  if (fs.existsSync(from)) fs.cpSync(from, to, { recursive: true, filter: (s) => !isLock(s) });
}
