/**
 * The factory: getConnection() builds the configured driver once and hands the
 * same one to every caller, and closeConnection() lets the next call build
 * afresh from the configuration as it is then.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-storage-factory-');
const { getConnection, getKeelConnection, closeConnection, createConnection } =
  await import('../../../../src/platform/storage/index.ts');

afterEach(async () => {
  restoreEnv('OYA_STORAGE', undefined);
  await closeConnection();
});

describe('getConnection', () => {
  it('builds the default SQLite driver once and shares it', () => {
    const first = getConnection();
    assert.equal(first.kind, 'sqlite');
    assert.equal(getConnection(), first);
  });

  it('builds afresh from the configuration after a close', async () => {
    getConnection();
    await closeConnection();
    process.env.OYA_STORAGE = 'file';
    assert.equal(getConnection().kind, 'file');
  });
});

describe('createConnection', () => {
  it('builds each configured driver', async () => {
    const postgres = createConnection({ kind: 'postgres', databaseUrl: 'postgres://unused/db' });
    assert.equal(postgres.kind, 'postgres');
    assert.equal(createConnection({ kind: 'file' }).kind, 'file');
  });
});

describe('getKeelConnection', () => {
  it('is the main connection when Keel has no database of its own, as on a self-hosted server', () => {
    assert.equal(getKeelConnection({ kind: 'sqlite' }), getConnection());
  });

  it("is a separate Postgres connection when KEEL_DATABASE_URL names Keel's database", () => {
    const keel = getKeelConnection({
      kind: 'postgres',
      databaseUrl: 'postgres://h/main',
      keelDatabaseUrl: 'postgres://h/keel',
    });
    assert.equal(keel.kind, 'postgres');
    assert.notEqual(keel, getConnection());
  });
});
