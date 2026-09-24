/**
 * Storage configuration: OYA_STORAGE picks the driver, SQLite by default, and
 * a configuration that would put data somewhere nobody meant is refused.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { storageConfig } from '../../../../src/platform/storage/index.ts';

/** A Postgres connection string that is never dialled. */
const URL = 'postgres://h/db';

describe('storageConfig', () => {
  it('defaults to SQLite', () => {
    assert.equal(storageConfig({}).kind, 'sqlite');
  });

  it('takes the driver from OYA_STORAGE, whatever its case or spacing', () => {
    assert.equal(storageConfig({ OYA_STORAGE: ' File ' }).kind, 'file');
    assert.deepEqual(storageConfig({ OYA_STORAGE: 'postgres', DATABASE_URL: URL }), {
      kind: 'postgres',
      databaseUrl: URL,
    });
  });

  it('refuses a driver there is none of, naming the ones there are', () => {
    assert.throws(() => storageConfig({ OYA_STORAGE: 'mongo' }), /one of postgres, sqlite, file, not "mongo"/);
  });

  it('refuses Postgres without DATABASE_URL', () => {
    assert.throws(() => storageConfig({ OYA_STORAGE: 'postgres' }), /needs DATABASE_URL/);
  });

  it('refuses DATABASE_URL without OYA_STORAGE=postgres, so an older Postgres deployment never lands on SQLite', () => {
    assert.throws(() => storageConfig({ DATABASE_URL: URL }), /DATABASE_URL is set but OYA_STORAGE is sqlite/);
    assert.throws(() => storageConfig({ DATABASE_URL: URL, OYA_STORAGE: 'file' }), /OYA_STORAGE is file/);
  });

  it('refuses Supabase sign-in on any driver but Postgres', () => {
    assert.throws(() => storageConfig({ SUPABASE_URL: 'https://x.supabase.co' }), /needs OYA_STORAGE=postgres/);
    storageConfig({ SUPABASE_URL: 'https://x.supabase.co', OYA_STORAGE: 'postgres', DATABASE_URL: URL });
  });
});
