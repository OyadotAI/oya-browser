/**
 * The single-writer lock on the SQLite control database.
 *
 * The failure this exists to prevent is a container that never comes back: the
 * server is pid 1 inside one, so an unclean stop leaves a lock naming pid 1, and
 * the next container, also pid 1, used to see that pid alive, assume another
 * server held the lock, and refuse to start. Forever.
 *
 * The lock must still turn away a second live replica sharing one volume, so
 * "always reclaim" is not the fix either. Both are checked here.
 */
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

process.env.OYA_PROFILE_SECRET ||= 'control-lock-tests';
const { SqliteBackend } = await import('../../src/modules/control/store.ts');

const dir = mkdtempSync(join(tmpdir(), 'oya-lock-'));
const dbPath = join(dir, 'control.sqlite');
const lockPath = `${dbPath}.lock`;

let passed = 0;
const ok = (label) => {
  passed++;
  console.log(`  ✅ ${label}`);
};

// ── A live holder is respected ──────────────────────────────────────────────
{
  const first = new SqliteBackend(dbPath);
  assert.throws(
    () => new SqliteBackend(dbPath),
    /already in use/,
    'a second writer against a freshly-held lock must be refused',
  );
  ok('a second writer is refused while the first holds the lock');

  const held = JSON.parse(readFileSync(lockPath, 'utf8'));
  assert.equal(held.pid, process.pid);
  assert.ok(Date.now() - held.at < 5000, 'the lock records when it was last refreshed');
  ok('the lock records a refresh time, not just a pid');

  first.close();
  assert.equal(existsSync(lockPath), false, 'a clean close releases the lock');
  ok('a clean close removes the lock');
}

// ── The container case: a stale lock naming pid 1 ───────────────────────────
{
  // Exactly what an unclean container stop leaves behind. pid 1 is alive, it is
  // this container's own init, so a pid-liveness check would refuse here.
  writeFileSync(lockPath, JSON.stringify({ pid: 1, at: Date.now() - 120_000 }));

  const back = new SqliteBackend(dbPath);
  ok('a stale lock naming pid 1 is reclaimed rather than bricking the deployment');
  assert.equal(JSON.parse(readFileSync(lockPath, 'utf8')).pid, process.pid, 'the lock is now ours');
  back.close();
}

// ── A fresh lock naming pid 1 still blocks ──────────────────────────────────
{
  // Same pid, but refreshed a moment ago: that is a live replica, not a corpse.
  writeFileSync(lockPath, JSON.stringify({ pid: 1, at: Date.now() }));
  assert.throws(() => new SqliteBackend(dbPath), /already in use/, 'a recently refreshed lock must still be respected');
  ok('a recently refreshed lock still blocks a second writer');
}

// ── A corrupt lock does not wedge the server ────────────────────────────────
{
  writeFileSync(lockPath, 'not json at all');
  const back = new SqliteBackend(dbPath);
  ok('an unreadable lock is treated as stale rather than fatal');
  back.close();
}

// ── The heartbeat actually advances ─────────────────────────────────────────
{
  const back = new SqliteBackend(dbPath);
  const first = JSON.parse(readFileSync(lockPath, 'utf8')).at;
  await new Promise((r) => setTimeout(r, 30));
  back.touchLock();
  const second = JSON.parse(readFileSync(lockPath, 'utf8')).at;
  assert.ok(second > first, `refresh must move forward (${first} -> ${second})`);
  // A stale read of a half-written record would be worse than no lock at all.
  assert.doesNotThrow(() => JSON.parse(readFileSync(lockPath, 'utf8')), 'the lock stays valid JSON when rewritten');
  ok('refreshing advances the timestamp and never leaves a torn record');
  back.close();
}

// ── Opting out of locking still works (tests, single-process tools) ─────────
{
  const a = new SqliteBackend(dbPath, { lock: false });
  const b = new SqliteBackend(dbPath, { lock: false });
  ok('lock:false allows the unlocked opens the test suite relies on');
  a.close();
  b.close();
}

console.log(`\n  ${passed} passed, 0 failed`);
