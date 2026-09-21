/**
 * Unit tests for the legacy ownership migration's steps: a private backup of
 * the data directory that never overwrites an earlier one, a project for every
 * key this process holds, and no database import without a database.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir, restoreEnv } from '../../../support/data-dir.ts';

ownDataDir('oya-control-migrate-steps-');
const savedKeys = process.env.API_KEYS;
process.env.API_KEYS = 'legacy-key';
const { assignOwners, backupData, importBrowsers } =
  await import('../../../../../src/modules/control/migrate/steps.ts');
const { projectId } = await import('../../../../../src/modules/control/service.ts');
const { scratchDir, scratchService } = await import('../../../support/control.ts');
restoreEnv('API_KEYS', savedKeys);

describe('backupData', () => {
  it('copies files and folders into a private backup, leaving out the database, backups and recordings', async () => {
    const data = scratchDir('oya-migrate-data-');
    writeFileSync(join(data, 'personas.json'), '{}');
    mkdirSync(join(data, 'profiles'));
    writeFileSync(join(data, 'profiles', 'p1.bin'), 'x');
    for (const skipped of ['control.sqlite', 'control.sqlite-wal', 'migration-backup-v0', 'recordings'])
      mkdirSync(join(data, skipped), { recursive: true });
    const backup = join(scratchDir('oya-migrate-backup-'), 'b');
    await backupData(data, backup);
    assert.deepEqual(readdirSync(backup).sort(), ['personas.json', 'profiles']);
    assert.equal(readFileSync(join(backup, 'profiles', 'p1.bin'), 'utf8'), 'x');
    assert.equal(statSync(backup).mode & 0o777, 0o700);
  });

  it('never overwrites what an earlier run copied', async () => {
    const data = scratchDir('oya-migrate-data-');
    const backup = join(scratchDir('oya-migrate-backup-'), 'b');
    writeFileSync(join(data, 'a.json'), 'first');
    await backupData(data, backup);
    writeFileSync(join(data, 'a.json'), 'second');
    await backupData(data, backup);
    assert.equal(readFileSync(join(backup, 'a.json'), 'utf8'), 'first');
  });
});

describe('assignOwners', () => {
  it('creates a project for each key held in the clear and maps it to its legacy owner', async () => {
    const service = scratchService();
    const mapping = await assignOwners(service);
    assert.deepEqual(mapping, [
      { project: projectId('legacy-key'), legacyOwner: (await service.project('legacy-key')).legacyOwner },
    ]);
    assert.equal((await service.project('legacy-key')).ownerUser, undefined, 'an env key has no account');
  });
});

describe('importBrowsers', () => {
  it('imports nothing without a database', async () => {
    const service = scratchService();
    await importBrowsers(service);
    assert.deepEqual(await service.store.list('session'), []);
  });
});
