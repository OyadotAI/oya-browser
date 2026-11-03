/** Exercises immutable daily values, enrichment, retry and mutable presence on real scratch storage. */
import { beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-keel-report-store-');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { recordKeelReport } = await import('../../../../src/modules/telemetry/keel-repository.ts');
const identity = {
  install_id: '6ed27761-8487-470f-b5ae-81d1886af2b4',
  version: '0.4.47',
  os: 'macos',
  license_id: 'lic-1',
};
const day = { day: '2026-11-01', steps: 10, runtime_ms: null, project_ids: null };

describe('Keel persistence', () => {
  beforeEach(async () => {
    await getConnection().delete('keel_usage', {});
    await getConnection().delete('keel_installs', {});
  });

  it('fills each first measurement once without rewriting legacy steps or original receipt', async () => {
    const received = '2026-11-02T01:00:00.000Z';
    await recordKeelReport({ identity, reports: [day] }, received);
    await recordKeelReport(
      { identity, reports: [{ ...day, steps: 999, runtime_ms: 1000 }] },
      '2026-11-03T01:00:00.000Z',
    );
    await recordKeelReport({ identity, reports: [{ ...day, runtime_ms: 2000, project_ids: ['a'.repeat(64)] }] });
    const [row] = await getConnection().select('keel_usage');
    assert.equal(row.steps, 10);
    assert.equal(row.received_at, received);
    assert.equal(row.runtime_ms, 1000);
    assert.deepEqual(row.project_ids, ['a'.repeat(64)]);
  });

  it('refreshes duplicate presence and replaces a current license with an explicit null', async () => {
    await recordKeelReport({ identity, reports: [day] }, '2026-11-02T01:00:00.000Z');
    await recordKeelReport({ identity: { ...identity, license_id: null }, reports: [day] }, '2026-11-03T01:00:00.000Z');
    const [seen] = await getConnection().select('keel_installs');
    assert.equal(seen.license_id, null);
    assert.equal(seen.first_seen, '2026-11-02T01:00:00.000Z');
    assert.equal(seen.last_seen, '2026-11-03T01:00:00.000Z');
    assert.equal((await getConnection().select('keel_usage')).length, 1);
  });

  it('does not regress current metadata when an older receipt finishes later', async () => {
    await recordKeelReport({ identity, reports: [] }, '2026-11-03T01:00:00.000Z');
    await recordKeelReport({ identity: { ...identity, license_id: 'old' }, reports: [] }, '2026-11-01T01:00:00.000Z');
    const [seen] = await getConnection().select('keel_installs');
    assert.equal(seen.license_id, 'lic-1');
    assert.equal(seen.first_seen, '2026-11-01T01:00:00.000Z');
    assert.equal(seen.last_seen, '2026-11-03T01:00:00.000Z');
  });

  it('deduplicates concurrent replay without adding counters', async () => {
    const report = { identity, reports: [{ ...day, runtime_ms: 1000, project_ids: [] }] };
    await Promise.all([recordKeelReport(report), recordKeelReport(report)]);
    const rows = await getConnection().select('keel_usage');
    assert.equal(rows.length, 1);
    assert.equal(rows[0].runtime_ms, 1000);
    assert.equal(rows[0].steps, 10);
  });
});
