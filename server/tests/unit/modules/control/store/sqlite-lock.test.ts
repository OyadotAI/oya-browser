/**
 * Unit tests for the SQLite single-writer lock file: claiming, refusing while a
 * live holder refreshes it, taking over a stale or unreadable one, releasing.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeSync, existsSync, readFileSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { claimLock, releaseLockFile, writeLockRecord } from '../../../../../src/modules/control/store/sqlite-lock.ts';
import { LOCK_STALE_MS } from '../../../../../src/modules/control/store/constants.ts';
import { scratchDir } from '../../../support/control.ts';

/** A lock path in a fresh directory. */
const lockPath = () => join(scratchDir('oya-lock-'), 'control.sqlite.lock');

describe('claimLock', () => {
  it('creates a lock file private to the server user', () => {
    const path = lockPath();
    const fd = claimLock(path);
    assert.equal(statSync(path).mode & 0o777, 0o600);
    releaseLockFile(fd, path);
  });

  it('refuses while another server keeps the lock fresh', () => {
    const path = lockPath();
    writeFileSync(path, JSON.stringify({ pid: 1, at: Date.now() }));
    assert.throws(() => claimLock(path), /already in use; use Postgres for multiple replicas/);
  });

  it('takes over a lock nobody has refreshed within the stale window, whatever pid it names', () => {
    const path = lockPath();
    writeFileSync(path, JSON.stringify({ pid: process.pid, at: Date.now() - LOCK_STALE_MS - 1 }));
    const fd = claimLock(path);
    releaseLockFile(fd, path);
  });

  it('takes over a lock whose record cannot be read', () => {
    const path = lockPath();
    writeFileSync(path, 'not json');
    releaseLockFile(claimLock(path), path);
  });

  it('passes on errors other than an existing lock', () => {
    assert.throws(() => claimLock(join(scratchDir(), 'missing', 'x.lock')), { code: 'ENOENT' });
  });
});

describe('writeLockRecord', () => {
  it('records this pid and the current time', () => {
    const path = lockPath();
    const fd = claimLock(path);
    writeLockRecord(fd);
    const record = JSON.parse(readFileSync(path, 'utf8'));
    assert.equal(record.pid, process.pid);
    assert.ok(Date.now() - record.at < LOCK_STALE_MS);
    releaseLockFile(fd, path);
  });

  it('never throws when the lock cannot be written', () => {
    const path = lockPath();
    const fd = claimLock(path);
    closeSync(fd);
    assert.doesNotThrow(() => writeLockRecord(fd));
  });
});

describe('releaseLockFile', () => {
  it('removes the lock so the next server can start at once', () => {
    const path = lockPath();
    releaseLockFile(claimLock(path), path);
    assert.equal(existsSync(path), false);
  });

  it('tolerates a lock file that is already gone', () => {
    const path = lockPath();
    const fd = claimLock(path);
    unlinkSync(path);
    assert.doesNotThrow(() => releaseLockFile(fd, path));
  });
});
