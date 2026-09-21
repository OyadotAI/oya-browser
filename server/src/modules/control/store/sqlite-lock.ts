/**
 * One SQLite control database allows exactly one writer, and the lock has to
 * survive the holder being killed.
 *
 * A pid is not an identity here. In a container the server is pid 1, so after an
 * unclean stop the lock names pid 1 and the *next* container, also pid 1, finds
 * that pid alive, concludes another server holds the lock, and refuses to start.
 * The deployment then never recovers. So the holder proves it is alive by
 * touching the lock instead: a lock nobody has refreshed is stale, whatever pid
 * it names, while a second live replica keeps its own lock fresh and is still
 * correctly turned away.
 */
import { openSync, closeSync, unlinkSync, readFileSync, writeSync, ftruncateSync } from 'node:fs';
import { LOCK_STALE_MS, MS_PER_SECOND, PRIVATE_FILE_MODE } from './constants.ts';

/** Create the lock file, taking over a stale one; throws while a live server holds it. Returns its descriptor. */
export function claimLock(path) {
  try {
    return openSync(path, 'wx', PRIVATE_FILE_MODE);
  } catch (err) {
    if (err.code !== 'EEXIST') throw err;
    refuseIfFresh(lockAge(path));
    unlinkSync(path);
    return openSync(path, 'wx', PRIVATE_FILE_MODE);
  }
}

/** The lock's recorded holder, or null when it is unreadable. */
function readHolder(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch {
    return null;
  }
}

/** Milliseconds since the holder last refreshed the lock; Infinity when it never did. */
function lockAge(path) {
  const held = readHolder(path);
  return held?.at ? Date.now() - Number(held.at) : Infinity;
}

/** Refuse to start while another server is still refreshing its lock. */
function refuseIfFresh(age) {
  if (!(age < LOCK_STALE_MS)) return;
  throw new Error(
    'Local control database is already in use; use Postgres for multiple replicas' +
      ` (lock refreshed ${Math.round(age / MS_PER_SECOND)}s ago; it goes stale after ${LOCK_STALE_MS / MS_PER_SECOND}s` +
      ' if that server is gone)',
  );
}

/** Rewrite the lock file with our pid and the current time, proving the holder is alive. */
export function writeLockRecord(fd) {
  try {
    const record = `${JSON.stringify({ pid: process.pid, at: Date.now() })}\n`;
    writeSync(fd, record, 0);
    ftruncateSync(fd, Buffer.byteLength(record));
  } catch {
    /* a lock we cannot refresh will be reclaimed; that is the intent */
  }
}

/** Close and remove the lock file. */
export function releaseLockFile(fd, path) {
  closeSync(fd);
  try {
    unlinkSync(path);
  } catch {
    /* already gone */
  }
}
