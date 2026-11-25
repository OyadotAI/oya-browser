/** Pins staff Keel totals, UTC windows, historical presence and honest coverage through its public service. */
import { beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-admin-keel-');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { keelOverview } = await import('../../../../src/modules/admin/service.ts');
const NOW = Date.parse('2026-11-03T12:00:00.000Z');

/** Seeds original rows to exercise local-driver migration rather than fabricated presence dates. */
async function usage(install_id: string, day: string, extra = {}) {
  await getConnection().upsert('keel_usage', [
    {
      install_id,
      day,
      steps: 10,
      version: '0.4.47',
      os: 'macos',
      license_id: 'lic-1',
      received_at: '2026-11-02T12:00:00.000Z',
      runtime_ms: null,
      project_ids: null,
      ...extra,
    },
  ]);
}

describe('Keel admin snapshot', () => {
  beforeEach(async () => {
    await getConnection().delete('keel_usage', {});
    await getConnection().delete('keel_installs', {});
  });

  it('distinguishes an empty history from measured zero', async () => {
    const empty = await keelOverview(NOW);
    assert.equal(empty.usage.runtime_ms, null);
    assert.equal(empty.usage.projects, null);
    assert.equal(empty.installs.total, 0);
    await usage('one', '2026-11-02', { runtime_ms: 0, project_ids: [] });
    const zero = await keelOverview(NOW);
    assert.equal(zero.usage.runtime_ms, 0);
    assert.equal(zero.usage.projects, 0);
  });

  it('backfills original receipt times and excludes presence markers, today and future days from usage', async () => {
    await usage('old', '', { steps: 0, received_at: '2026-09-01T00:00:00.000Z' });
    await usage('current', '2026-11-02');
    await usage('current', '2026-11-03', { steps: 999 });
    await usage('future', '2099-01-01', { steps: 999, received_at: '2026-09-01T00:00:00.000Z' });
    const view = await keelOverview(NOW);
    assert.equal(view.installs.total, 3);
    assert.equal(view.installs.active, 1);
    assert.equal(view.usage.reports, 1);
    assert.equal(view.usage.steps, 10);
    assert.deepEqual(view.period, { from: '2026-10-04', to: '2026-11-02', days: 30, active_days: 7 });
    assert.equal(view.days.length, 30);
    assert.equal(view.installs.list.find((row) => row.install_id === 'old')?.first_seen, '2026-09-01T00:00:00.000Z');
  });

  it('deduplicates projects across days per installation and exposes partial measurement coverage', async () => {
    await usage('one', '2026-11-01', { runtime_ms: 1000, project_ids: ['a'] });
    await usage('one', '2026-11-02', { runtime_ms: 2000, project_ids: ['a', 'b'] });
    await usage('two', '2026-11-02', { project_ids: ['a'] });
    const view = await keelOverview(NOW);
    assert.equal(view.usage.runtime_ms, 3000);
    assert.equal(view.usage.runtime_reports, 2);
    assert.equal(view.usage.project_reports, 3);
    assert.equal(view.usage.projects, 3);
    assert.equal(view.usage.reports, 3);
    assert.equal(view.licenses.total, 1);
    assert.equal(view.licenses.list[0].installs, 2);
  });

  it('counts only the latest current license and preserves a removed license as null', async () => {
    await usage('one', '2026-11-01', { license_id: 'old', received_at: '2026-11-01T00:00:00.000Z' });
    await usage('one', '2026-11-02', { license_id: null });
    const view = await keelOverview(NOW);
    assert.equal(view.licenses.total, 0);
    assert.equal(view.installs.list[0].license_id, null);
  });

  it('does not call a future server receipt active after a clock correction', async () => {
    await usage('future-receipt', '2026-11-02', { received_at: '2026-11-04T00:00:00Z' });
    const view = await keelOverview(NOW);
    assert.equal(view.installs.total, 1);
    assert.equal(view.installs.active, 0);
    assert.equal(view.licenses.total, 0);
  });

  it('bounds identity tables without truncating headline totals', async () => {
    for (let index = 0; index < 55; index++)
      await usage(`install-${index}`, '2026-11-02', { license_id: `lic-${index}` });
    const view = await keelOverview(NOW);
    assert.equal(view.installs.total, 55);
    assert.equal(view.licenses.total, 55);
    assert.equal(view.installs.list.length, 50);
    assert.equal(view.licenses.list.length, 50);
  });
});
