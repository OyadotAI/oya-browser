/**
 * Unit tests for the legacy ownership migration: it backs up the data
 * directory, writes the ownership mapping beside the backup, and marks itself
 * done so it never runs twice.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-control-migrate-');
const { migrateLegacy } = await import('../../../../src/modules/control/migrate.ts');
const { control } = await import('../../../../src/modules/control/service.ts');

describe('migrateLegacy', () => {
  it('backs up, writes a private ownership mapping, and marks the migration done', async () => {
    writeFileSync(join(dir, 'personas.json'), '{}');
    await migrateLegacy();
    const backup = join(dir, 'migration-backup-v1');
    assert.ok(existsSync(join(backup, 'personas.json')));
    assert.ok(Array.isArray(JSON.parse(readFileSync(join(backup, 'ownership.json'), 'utf8'))));
    assert.equal(statSync(join(backup, 'ownership.json')).mode & 0o777, 0o600);
    assert.equal((await control().store.get('meta', 'migrations')).legacyOwnership, 1);
  });

  it('does nothing once it has run', async () => {
    const mapping = join(dir, 'migration-backup-v1', 'ownership.json');
    writeFileSync(mapping, 'untouched');
    await migrateLegacy();
    assert.equal(readFileSync(mapping, 'utf8'), 'untouched');
  });
});
