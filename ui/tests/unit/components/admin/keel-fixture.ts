/** Synthetic anonymous Keel snapshots shared by view and asynchronous-feed tests. */
import type { KeelSnapshot } from '@/components/admin/keel-types';

/** A partial-reporting snapshot uses no real installation or license details. */
export const KEEL_SNAPSHOT: KeelSnapshot = {
  generated_at: '2026-11-03T12:00:00.000Z',
  latest_report: '2026-11-03T10:00:00.000Z',
  period: { from: '2026-10-04', to: '2026-11-02', days: 30, active_days: 7 },
  usage: { reports: 2, runtime_reports: 1, project_reports: 1, runtime_ms: 3_600_000, projects: 2, steps: 200 },
  days: [
    {
      day: '2026-11-02',
      reports: 2,
      runtime_reports: 1,
      project_reports: 1,
      runtime_ms: 3_600_000,
      projects: 2,
      steps: 200,
    },
  ],
  installs: {
    total: 117,
    active: 37,
    list: [
      {
        install_id: '6ed27761-8487-470f-b5ae-81d1886af2b4',
        version: '0.4.47',
        os: 'macos',
        license_id: null,
        first_seen: '2026-10-01T00:00:00.000Z',
        last_seen: '2026-11-03T10:00:00.000Z',
        reports: 2,
        runtime_reports: 1,
        project_reports: 1,
        runtime_ms: 3_600_000,
        projects: 2,
        steps: 200,
      },
    ],
  },
  licenses: {
    total: 5,
    list: [{ license_id: 'synthetic-license-id', installs: 2, last_seen: '2026-11-03T10:00:00.000Z' }],
  },
};
