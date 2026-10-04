/**
 * Unit tests for the overflow import: spilled rows reach audit_log with their
 * chain links, a rerun adds nothing twice, and a row already stored is skipped.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, rmSync } from 'node:fs';
import { OVERFLOW_FILE, audit, drain, verifyStored } from '../../../src/platform/audit.ts';
import { importOverflow } from '../../../src/platform/audit-import.ts';
import { getConnection } from '../../../src/platform/storage/index.ts';
import { AUDIT_PENDING_MAX } from '../../../src/platform/constants.ts';

/** Spills `count` events to the overflow file by holding storage down past the backlog bound. */
async function spill(count: number) {
  await drain();
  rmSync(OVERFLOW_FILE, { force: true });
  const error = mock.method(console, 'error', () => {});
  const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
  for (let i = 0; i < AUDIT_PENDING_MAX + count; i++) audit({ action: 'import.test', targetId: String(i) });
  upsert.mock.restore();
  error.mock.restore();
  await drain();
}

describe('importOverflow', () => {
  it('loads spilled rows so the chain verifies without a gap, and a rerun adds nothing', async () => {
    await spill(3);
    assert.equal(readFileSync(OVERFLOW_FILE, 'utf8').trim().split('\n').length, 3);
    assert.deepEqual(await importOverflow(), { read: 3, imported: 3, skipped: 0 });
    assert.deepEqual(await importOverflow(), { read: 3, imported: 0, skipped: 3 });
    const stored = await getConnection().select('audit_log', { action: 'import.test' });
    assert.equal(stored.length, AUDIT_PENDING_MAX + 3);
    assert.equal((await verifyStored(null)).ok, true);
  });

  it('skips a row of a partly imported file that is already stored', async () => {
    await spill(2);
    const [first] = readFileSync(OVERFLOW_FILE, 'utf8').trim().split('\n');
    await getConnection().upsert('audit_log', [JSON.parse(first)]);
    assert.deepEqual(await importOverflow(OVERFLOW_FILE), { read: 2, imported: 1, skipped: 1 });
  });
});
