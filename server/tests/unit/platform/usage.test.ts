/**
 * Unit tests for per-key usage accounting: hourly counters keyed by key
 * fingerprint, browser-seconds for connected browsers, the hour rollover, and
 * the usage table they are written to, read back from and restored out of.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import * as usage from '../../../src/platform/usage.ts';
import { fingerprint } from '../../../src/platform/audit.ts';
import { getConnection } from '../../../src/platform/storage/index.ts';
import { MS_PER_HOUR, MS_PER_SECOND } from '../../../src/platform/constants.ts';

/** A browser's connected time in the drain test. */
const CONNECTED_SECONDS = 30;

/** The stored row for a key, or undefined. */
const storedRow = async (key: string) => (await getConnection().select('usage', { api_key: fingerprint(key) }))[0];

/** 10:15 UTC on a fixed day, so the hour bucket is predictable. */
const T0 = Date.UTC(2026, 0, 1, 10, 15);

describe('usage', () => {
  beforeEach(async () => {
    mock.timers.enable({ apis: ['Date'], now: T0 });
    usage.reset();
    await getConnection().delete('usage', {});
  });
  afterEach(() => mock.timers.reset());

  it('adds to a counter for the key’s current hour', () => {
    usage.record('key-a', 'commands');
    usage.record('key-a', 'commands', 2);
    const now = usage.current('key-a');
    assert.equal(now.commands, 3);
    assert.equal(now.hour, '2026-01-01T10:00:00.000Z');
  });

  it('ignores unknown fields, non-numbers and a missing key', () => {
    usage.record('key-a', 'not_a_field');
    usage.record('key-a', 'commands', NaN);
    usage.record('', 'commands');
    assert.equal(usage.current('key-a').commands, 0);
    assert.equal(usage.snapshot().length, 0);
  });

  it('reads an earlier hour’s counters as zero, so an hourly block lifts', () => {
    usage.record('key-a', 'sandboxes_created', 5);
    mock.timers.tick(MS_PER_HOUR);
    assert.equal(usage.current('key-a').sandboxes_created, 0);
  });

  it('starts a fresh bucket on the first write of a new hour', () => {
    usage.record('key-a', 'commands', 4);
    mock.timers.tick(MS_PER_HOUR);
    usage.record('key-a', 'commands');
    assert.equal(usage.current('key-a').commands, 1);
  });

  it('counts a connected browser as started and open', () => {
    usage.browserConnected('key-a', 'b1');
    const now = usage.current('key-a');
    assert.equal(now.browsers_started, 1);
    assert.equal(now.openBrowsers, 1);
  });

  it('books the seconds a browser was connected when it disconnects', () => {
    usage.browserConnected('key-a', 'b1');
    mock.timers.tick(90 * MS_PER_SECOND);
    usage.browserDisconnected('key-a', 'b1');
    const now = usage.current('key-a');
    assert.equal(now.browser_seconds, 90);
    assert.equal(now.openBrowsers, 0);
  });

  it('ignores a disconnect for a browser it never saw', () => {
    usage.browserDisconnected('key-a', 'ghost');
    usage.browserDisconnected('', 'ghost');
    assert.equal(usage.current('key-a').browser_seconds, 0);
  });

  it('lists activity by a short fingerprint, never the key itself', () => {
    usage.record('secret-key', 'commands');
    const [row] = usage.snapshot();
    assert.equal(row.actor, fingerprint('secret-key').slice(0, 12));
    assert.ok(!JSON.stringify(row).includes('secret-key'));
  });

  it('answers history from the usage table, newest hour first', async () => {
    usage.record('key-a', 'commands');
    await usage.drain();
    const out = await usage.history('key-a');
    assert.equal(out.source, 'database');
    assert.equal(out.rows[0].commands, 1);
    assert.equal(out.rows[0].hour, '2026-01-01T10:00:00.000Z');
  });

  it('answers history from the live bucket, with the error, when storage cannot be read', async () => {
    usage.record('key-a', 'commands');
    const select = mock.method(getConnection(), 'select', async () => Promise.reject(new Error('storage down')));
    const out = await usage.history('key-a');
    select.mock.restore();
    assert.deepEqual([out.source, out.error, out.rows[0].commands], ['memory', 'storage down', 1]);
  });

  it('continues the current hour’s counters after a restart, so a quota cannot be reset by one', async () => {
    usage.record('key-a', 'commands', 2);
    await usage.drain();
    usage.reset();
    await usage.restore();
    assert.equal(usage.current('key-a').commands, 2);
  });

  it('restores nothing from an earlier hour', async () => {
    usage.record('key-a', 'commands', 2);
    await usage.drain();
    usage.reset();
    mock.timers.tick(MS_PER_HOUR);
    await usage.restore();
    assert.equal(usage.snapshot().length, 0);
  });

  it('settles open browsers and writes their seconds on drain', async () => {
    usage.browserConnected('key-a', 'b1');
    mock.timers.tick(CONNECTED_SECONDS * MS_PER_SECOND);
    await usage.drain();
    const row = await storedRow('key-a');
    assert.deepEqual([row.browser_seconds, row.browsers_started], [CONNECTED_SECONDS, 1]);
  });

  it('keeps counters a failed write could not store, logs it, and writes them on the next flush', async () => {
    const error = mock.method(console, 'error', () => {});
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    usage.record('key-a', 'commands');
    await usage.drain();
    upsert.mock.restore();
    assert.match(error.mock.calls[0].arguments[0], /write failed \(storage down\)/);
    await usage.drain();
    assert.equal((await storedRow('key-a')).commands, 1);
    error.mock.restore();
  });
});
