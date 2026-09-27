/**
 * Unit tests for what the admin page asks the server: the overview, a person
 * by email, and licenses issued and revoked. Over the scratch database.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-admin-service-');
const admin = await import('../../../../src/modules/admin/service.ts');
const repo = await import('../../../../src/modules/admin/repository.ts');
const license = await import('../../../../src/platform/license/index.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { countDownload, flushDownloads } = await import('../../../../src/modules/admin/download-counter.ts');

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);

/** A licensing module that signs nothing but says what it was asked. */
const FAKE = {
  start() {},
  admit: () => ({ ok: true, message: '' }),
  mint: (req: any) => ({
    license: { id: 'L1', licensee: req.licensee, maxConcurrent: 9, expiresAt: '2030-01-01T00:00:00.000Z' },
    token: 'KEY',
  }),
};

describe('admin service', () => {
  afterEach(() => {
    license.setModuleForTests(null);
    delete process.env.OYA_LICENSE_SIGNING_KEY;
  });

  it('issues a license, answers its key once, and keeps it without the key', async () => {
    license.setModuleForTests(FAKE);
    process.env.OYA_LICENSE_SIGNING_KEY = 'k';
    const issued = await admin.issue({ licensee: 'Acme' }, 'mk@getoya.ai');
    assert.equal(issued.key, 'KEY');
    const [stored] = await admin.listLicenses();
    assert.equal(stored.licensee, 'Acme');
    assert.equal(stored.created_by, 'mk@getoya.ai');
    assert.ok(!JSON.stringify(stored).includes('KEY'));
  });

  it('revokes a license once, and says so to the install that holds it', async () => {
    const first = await admin.revoke('L1');
    const again = await admin.revoke('L1');
    assert.equal(first.revoked_at, again.revoked_at);
    assert.equal(await admin.isRevoked('L1'), true);
    assert.equal(await admin.isRevoked(null), false);
    await assert.rejects(admin.revoke('nope'), { status: 404 });
  });

  it('looks a person up by email: plan, usage and key prefixes, never keys', async () => {
    await getConnection().upsert('profiles', [
      { id: 'u1', email: 'ana@example.com', created_at: '2026-03-01T00:00:00Z' },
    ]);
    await getConnection().upsert('api_keys', [
      { key_hash: 'h1', key_prefix: 'abc', user_id: 'u1', label: 'CI', created_at: '2026-03-01T00:00:00Z' },
    ]);
    const found = await admin.lookup(' Ana@Example.com ', NOW);
    assert.equal(found.standing.plan, 'free');
    assert.deepEqual(
      found.keys.map((k) => k.prefix),
      ['abc'],
    );
    assert.ok(!JSON.stringify(found).includes('h1'));
    await assert.rejects(admin.lookup('nobody@example.com', NOW), { status: 404 });
  });

  it('builds the overview from accounts, installs, downloads and the fleet', async () => {
    await repo.recordPing(
      { id: 'i1', version: '1.0.0', browsers: 2, peak_cloud: 8, license: null },
      new Date(NOW).toISOString(),
    );
    countDownload('installer', 'mac', new Date(NOW));
    countDownload('installer', 'mac', new Date(NOW));
    await flushDownloads();
    const o = await admin.overview(NOW);
    assert.equal(o.accounts.total, 1);
    assert.deepEqual(o.installs.total, 1);
    assert.equal(o.installs.overCap, 1);
    assert.deepEqual(
      o.downloads.map((d) => [d.kind, d.count]),
      [['installer', 2]],
    );
    assert.equal(typeof o.fleet.total, 'number');
  });

  it('adds later downloads to the same day', async () => {
    countDownload('installer', 'mac', new Date(NOW));
    await flushDownloads();
    const [row] = await getConnection().select('download_counts', {
      day: '2026-03-15',
      kind: 'installer',
      platform: 'mac',
    });
    assert.equal(row.count, 3);
  });

  it('answers whether a ping is an install’s first today', async () => {
    const day = new Date(NOW).toISOString();
    assert.equal(
      await repo.recordPing({ id: 'i2', version: '1', browsers: 0, peak_cloud: 0, license: null }, day),
      true,
    );
    assert.equal(
      await repo.recordPing({ id: 'i2', version: '1', browsers: 0, peak_cloud: 0, license: null }, day),
      false,
    );
  });
});

describe('download counter', () => {
  it('keeps counts a failed write could not store, and writes them next time', async () => {
    const { mock } = await import('node:test');
    const conn = getConnection();
    const upsert = mock.method(conn, 'upsert', async () => {
      throw new Error('disk full');
    });
    mock.method(console, 'error', () => {});
    countDownload('update_check', 'linux', new Date(NOW));
    await flushDownloads();
    upsert.mock.restore();
    await flushDownloads();
    const [row] = await getConnection().select('download_counts', {
      day: '2026-03-15',
      kind: 'update_check',
      platform: 'linux',
    });
    assert.equal(row.count, 1);
    mock.restoreAll();
  });

  it('writes nothing when nothing was counted, and drains on shutdown', async () => {
    const { drainDownloads } = await import('../../../../src/modules/admin/download-counter.ts');
    await flushDownloads();
    await drainDownloads();
  });
});
