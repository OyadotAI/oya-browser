/** Restartable ownership migration. Existing encrypted profile contexts remain unchanged. */
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { control } from './service.ts';
import { dataPath } from '../../platform/paths.ts';
import { assignOwners, backupData, importBrowsers } from './migrate/steps.ts';
import { MAPPING_FILE_MODE, MAPPING_INDENT } from './migrate/constants.ts';

/** Write which projects were given owners beside the backup. */
async function writeMapping(backup, mapping) {
  await writeFile(join(backup, 'ownership.json'), JSON.stringify(mapping, null, MAPPING_INDENT), {
    mode: MAPPING_FILE_MODE,
  });
}

/** Record the migration as done, so it never runs again. */
async function markMigrated(service) {
  await service.store.transact(async (tx) => {
    const migrations = await tx.get('meta', 'migrations');
    tx.put('meta', 'migrations', { ...migrations, id: 'migrations', legacyOwnership: 1 });
  });
}

/**
 * Runs once (marked in meta.migrations): backs up the data directory, gives each legacy
 * project its owning user and imports legacy browser rows as disconnected sessions.
 */
export async function migrateLegacy() {
  const service = control();
  if ((await service.store.get('meta', 'migrations'))?.legacyOwnership === 1) return;
  const backup = dataPath('migration-backup-v1');
  await backupData(dataPath(), backup);
  const mapping = await assignOwners(service);
  await importBrowsers(service);
  await writeMapping(backup, mapping);
  await markMigrated(service);
}
