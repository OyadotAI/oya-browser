/**
 * Copying profile files safely: skip a source's lock files, and copy only what
 * is there. Shared by the capture snapshot and the partition seeding.
 */
const fs = require('fs');

/** A LevelDB/lock file, skipped so a copy never carries the source's lock. */
const isLock = (src) => /(^|[\\/])(LOCK|lockfile)$/i.test(src);

/** Copies one file or directory tree when it exists, leaving lock files behind. */
function copyIfPresent(from, to) {
  if (fs.existsSync(from)) fs.cpSync(from, to, { recursive: true, filter: (s) => !isLock(s) });
}

module.exports = { isLock, copyIfPresent };
