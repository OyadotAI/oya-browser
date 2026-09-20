/**
 * Unit tests for per-key usage accounting: hourly counters keyed by key
 * fingerprint, browser-seconds for connected browsers, the hour rollover, and
 * the file written when there is no database.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as usage from '../../../src/platform/usage.ts';
import { fingerprint } from '../../../src/platform/audit.ts';
import { dataPath } from '../../../src/platform/paths.ts';
import { MS_PER_HOUR, MS_PER_SECOND } from '../../../src/platform/constants.ts';

/** 10:15 UTC on a fixed day, so the hour bucket is predictable. */
const T0 = Date.UTC(2026, 0, 1, 10, 15);

describe('usage', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['Date'], now: T0 });
    usage.reset();
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

  it('answers history from memory when there is no database', async () => {
    usage.record('key-a', 'commands');
    const out = await usage.history('key-a');
    assert.equal(out.source, 'memory');
    assert.equal(out.rows[0].commands, 1);
  });

  it('does nothing on restore without a database', async () => {
    await usage.restore();
    assert.equal(usage.snapshot().length, 0);
  });

  it('settles open browsers and writes usage.json on drain', async () => {
    usage.browserConnected('key-a', 'b1');
    mock.timers.tick(30 * MS_PER_SECOND);
    await usage.drain();
    const rows = JSON.parse(readFileSync(dataPath('usage.json'), 'utf8'));
    const mine = rows.find((r) => r.api_key === fingerprint('key-a'));
    assert.equal(mine.browser_seconds, 30);
    assert.equal(mine.browsers_started, 1);
  });
});
