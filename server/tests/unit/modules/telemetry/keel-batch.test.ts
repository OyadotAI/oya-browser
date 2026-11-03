/** Pins the backward-compatible telemetry boundary and each v2 size/type refusal. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseKeelReport } from '../../../../src/modules/telemetry/keel-batch.ts';

const IDENTITY = {
  install_id: '6ed27761-8487-470f-b5ae-81d1886af2b4',
  version: '0.4.47',
  os: 'macos',
  license_id: null,
};
const DAY = { day: '2026-11-01', steps: 20, runtime_ms: 1000, project_ids: ['a'.repeat(64)] };
const NOW = '2026-11-03T12:00:00.000Z';
const batch = (reports = [DAY]) => ({ ...IDENTITY, schema_version: 2, reports });

describe('Keel batch validation', () => {
  it('preserves legacy counters and leaves their measurements unavailable', () => {
    const { identity, reports } = parseKeelReport({ ...IDENTITY, day: '', steps: 0 }, NOW);
    assert.deepEqual(identity, IDENTITY);
    assert.deepEqual(reports, [{ day: '', steps: 0, runtime_ms: null, project_ids: null }]);
  });

  it('preserves independent null, measured zero and opaque project sets', () => {
    const input = batch([
      { ...DAY, runtime_ms: 0, project_ids: [] },
      { ...DAY, day: '2026-11-02', runtime_ms: null, project_ids: null },
    ]);
    assert.deepEqual(parseKeelReport(input, NOW).reports, input.reports);
    assert.deepEqual(parseKeelReport(batch([]), NOW).reports, []);
  });

  it('discards unknown fields at both envelope and daily boundaries', () => {
    const report = parseKeelReport({ ...batch(), path: '/private', reports: [{ ...DAY, prompt: 'secret' }] }, NOW);
    assert.deepEqual(report, { identity: IDENTITY, reports: [DAY] });
  });

  it('refuses incomplete, future, duplicate or excessive daily snapshots', () => {
    for (const reports of [
      [{ ...DAY, day: '' }],
      [{ ...DAY, day: '2026-11-03' }],
      [{ ...DAY, day: '2099-01-01' }],
      [DAY, DAY],
      Array.from({ length: 8 }, () => DAY),
      null,
      'days',
    ])
      assert.throws(() => parseKeelReport({ ...batch(), reports }, NOW));
  });

  it('refuses malformed runtime without numeric coercion', () => {
    for (const runtime_ms of [-1, 0.5, '1000', undefined, {}, 86_400_000 * 4096 + 1])
      assert.throws(() => parseKeelReport(batch([{ ...DAY, runtime_ms } as any]), NOW), { field: 'runtime_ms' });
  });

  it('refuses paths, duplicate identities and project collection overflow', () => {
    for (const project_ids of [
      undefined,
      ['/private/repo'],
      ['a'.repeat(64), 'a'.repeat(64)],
      Array(129).fill('a'.repeat(64)),
      'x',
    ])
      assert.throws(() => parseKeelReport(batch([{ ...DAY, project_ids } as any]), NOW), { field: 'project_ids' });
  });

  it('refuses unknown schema versions and oversized content even when that content would be discarded', () => {
    assert.throws(() => parseKeelReport({ ...batch(), schema_version: 3 }, NOW), { field: 'schema_version' });
    assert.throws(() => parseKeelReport({ ...batch(), ignored: 'x'.repeat(65_536) }, NOW), { field: 'body' });
  });
});
