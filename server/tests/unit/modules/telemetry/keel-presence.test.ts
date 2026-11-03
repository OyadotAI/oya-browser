/** Both local drivers backfill historical presence without creating migration-time activity. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { FileConnection } from '../../../../src/platform/storage/file.ts';
import { SqliteConnection } from '../../../../src/platform/storage/sqlite.ts';
import { initializeKeelInstalls, recordKeelPresence } from '../../../../src/modules/telemetry/keel-presence.ts';

for (const kind of ['file', 'sqlite'])
  describe(`${kind} Keel presence migration`, () => {
    it('retains original first/latest receipt and a latest removed license across repeated initialization', async () => {
      const directory = mkdtempSync(join(tmpdir(), 'oya-keel-backfill-'));
      const db =
        kind === 'file' ? new FileConnection(directory) : new SqliteConnection(join(directory, 'storage.sqlite'));
      try {
        const base = { install_id: 'one', version: '0.4.47', os: 'macos', steps: 1 };
        await db.upsert('keel_usage', [
          { ...base, day: '2026-09-01', license_id: 'old', received_at: '2026-09-02T00:00:00.000Z' },
          { ...base, day: '2026-10-01', license_id: null, received_at: '2026-10-02T00:00:00.000Z' },
        ]);
        await initializeKeelInstalls(db);
        assert.deepEqual(await db.select('keel_installs'), [
          {
            install_id: 'one',
            version: '0.4.47',
            os: 'macos',
            license_id: null,
            first_seen: '2026-09-02T00:00:00.000Z',
            last_seen: '2026-10-02T00:00:00.000Z',
          },
        ]);
        await recordKeelPresence(
          { install_id: 'one', version: '0.4.48', os: 'macos', license_id: 'current' },
          '2026-11-03T00:00:00.000Z',
          db,
        );
        await initializeKeelInstalls(db);
        const [current] = await db.select('keel_installs');
        assert.equal(current.license_id, 'current');
        assert.equal(current.last_seen, '2026-11-03T00:00:00.000Z');
        assert.equal(current.first_seen, '2026-09-02T00:00:00.000Z');
      } finally {
        await db.close();
        rmSync(directory, { recursive: true, force: true });
      }
    });
  });
