/**
 * Seeding one persona's Electron partition with a real profile's site storage
 * (localStorage, IndexedDB, service workers). Cookies reach the pool over the
 * socket, but these are files, so they are copied straight into the partition
 * before its tabs open. Only the desktop's own persona is seeded; a remote
 * browser cannot read local files, and gets its logins from the cookie pool.
 */
const path = require('path');
const { copyIfPresent } = require('./copy.cjs');
const { PROFILE_STORES } = require('./constants.cjs');

/** The on-disk directory Electron keeps a persistent partition's data in. */
function partitionDir(electron, personaId) {
  return path.join(electron.app.getPath('userData'), 'Partitions', `oya-${personaId}`);
}

/** Copies a real profile's storage directories into the persona's partition. */
function seedStorage(electron, personaId, userDataDir, profileDir) {
  const dest = partitionDir(electron, personaId);
  const from = path.join(userDataDir, profileDir);
  for (const store of PROFILE_STORES) copyIfPresent(path.join(from, store), path.join(dest, store));
}

module.exports = { seedStorage };
