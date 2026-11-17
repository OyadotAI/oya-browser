/**
 * Unit tests for a playbook's run history (src/modules/playbooks/history.ts):
 * newest first and capped, errors cut short, sealed per project, moved on rename,
 * dropped on delete, and a write that lost a race to another replica retried.
 * Storage is the scratch SQLite database.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-playbook-runs-');
const history = await import('../../../../src/modules/playbooks/history.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { RUN_HISTORY_KEPT, RUN_ERROR_MAX } = await import('../../../../src/modules/playbooks/constants.ts');

const OWNER = 'owner-a';
const OTHER = 'owner-b';

/** A finished run that ended at minute `n`. */
const run = (n: number, status: 'succeeded' | 'failed' = 'succeeded') => ({
  status,
  startedAt: `2026-11-17T10:${String(n).padStart(2, '0')}:00.000Z`,
  endedAt: `2026-11-17T10:${String(n).padStart(2, '0')}:30.000Z`,
  browserId: 'b1',
});

describe('playbook run history', () => {
  beforeEach(async () => {
    await getConnection().delete('playbook_runs', {});
  });
  afterEach(() => mock.restoreAll());

  it('keeps runs newest first, and only the newest RUN_HISTORY_KEPT', async () => {
    for (let n = 0; n < RUN_HISTORY_KEPT + 2; n++) await history.record(OWNER, 'p', run(n));
    const runs = await history.runsOf(OWNER, 'p');
    assert.equal(runs.length, RUN_HISTORY_KEPT);
    assert.equal(runs[0].startedAt, run(RUN_HISTORY_KEPT + 1).startedAt);
  });

  it('cuts a failed run’s error short, since page text can end up in it', async () => {
    await history.record(OWNER, 'p', { ...run(1, 'failed'), error: 'x'.repeat(RUN_ERROR_MAX * 2) });
    assert.equal((await history.runsOf(OWNER, 'p'))[0].error?.length, RUN_ERROR_MAX);
  });

  it('is sealed per project: another project reads nothing, and the stored value is not plain text', async () => {
    await history.record(OWNER, 'p', { ...run(1, 'failed'), error: 'patient Jane Roe' });
    assert.deepEqual(await history.runsOf(OTHER, 'p'), []);
    const [row] = await getConnection().select('playbook_runs', { owner: OWNER });
    assert.doesNotMatch(String(row.value), /Jane Roe/);
  });

  it('answers each playbook’s newest run in one read', async () => {
    await history.record(OWNER, 'a', run(1));
    await history.record(OWNER, 'a', run(2, 'failed'));
    await history.record(OWNER, 'b', run(3));
    const last = await history.lastRuns(OWNER);
    assert.deepEqual([last.get('a')?.status, last.get('b')?.status, last.has('c')], ['failed', 'succeeded', false]);
  });

  it('moves the history with a rename, and drops it with a delete', async () => {
    await history.record(OWNER, 'old', run(1));
    await history.move(OWNER, 'old', 'new');
    assert.deepEqual(
      [(await history.runsOf(OWNER, 'old')).length, (await history.runsOf(OWNER, 'new')).length],
      [0, 1],
    );
    await history.forget(OWNER, 'new');
    assert.deepEqual(await history.runsOf(OWNER, 'new'), []);
  });

  it('retries a write that another replica beat, so both runs land', async () => {
    await history.record(OWNER, 'p', run(1));
    const db = getConnection();
    const real = db.update.bind(db);
    let first = true;
    mock.method(db, 'update', async (...args: Parameters<typeof db.update>) => {
      if (first) return ((first = false), await history.record(OWNER, 'p', run(2)), 0);
      return real(...args);
    });
    await history.record(OWNER, 'p', run(3));
    assert.deepEqual(
      (await history.runsOf(OWNER, 'p')).map((r) => r.startedAt),
      [run(3).startedAt, run(2).startedAt, run(1).startedAt],
    );
  });

  it('two first runs finishing at once both land', async () => {
    await Promise.all([history.record(OWNER, 'p', run(1)), history.record(OWNER, 'p', run(2))]);
    assert.equal((await history.runsOf(OWNER, 'p')).length, 2);
  });

  it('a rename replaces a history left under the new name, rather than dropping the moved one', async () => {
    await history.record(OWNER, 'old', run(1));
    await history.record(OWNER, 'new', run(2, 'failed'));
    await history.move(OWNER, 'old', 'new');
    assert.deepEqual(
      (await history.runsOf(OWNER, 'new')).map((r) => r.startedAt),
      [run(1).startedAt],
    );
  });
});
