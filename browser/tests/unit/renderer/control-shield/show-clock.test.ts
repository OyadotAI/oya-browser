/**
 * Unit tests for the show's clock, without a page: a stale show's timers do
 * nothing, a scan waits for a pass still crossing, and the reveal waits for
 * the reading beam to end a sweep.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { RendererConstants as C } from '../../../../src/renderer/core/constants.ts';
import { ShowClock } from '../../../../src/renderer/control-shield/show-clock.ts';

describe('the show clock', () => {
  let clock: ShowClock;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 1_000_000 });
    clock = new ShowClock();
  });
  afterEach(() => mock.timers.reset());

  it('drops a timer set before the show was bumped', () => {
    const ran: string[] = [];
    clock.later(10, () => ran.push('old'));
    clock.bump();
    clock.later(10, () => ran.push('new'));
    mock.timers.tick(10);
    assert.deepEqual(ran, ['new']);
  });

  it('lets a scan start at once with no pass crossing, and waits out one that is', () => {
    assert.equal(clock.scanWait(), 0);
    clock.revealing();
    mock.timers.tick(C.SHIELD_REVEAL_MS / 2);
    assert.equal(clock.scanWait(), C.SHIELD_REVEAL_MS / 2);
  });

  it('starts the reveal at the end of the first sweep past the scan minimum', () => {
    assert.equal(clock.untilReveal(), 0, 'no scan, no wait');
    clock.reading();
    const sweeps = Math.ceil(C.SHIELD_MIN_SCAN_MS / C.SHIELD_SCAN_LOOP_MS);
    assert.equal(clock.untilReveal(), sweeps * C.SHIELD_SCAN_LOOP_MS);
    mock.timers.tick(sweeps * C.SHIELD_SCAN_LOOP_MS + 1);
    assert.equal(clock.untilReveal(), C.SHIELD_SCAN_LOOP_MS - 1, 'a scan past its minimum finishes its sweep');
  });

  it('forgets the scan and the pass once stopped', () => {
    clock.reading();
    clock.stop();
    assert.equal(clock.untilReveal(), 0);
    assert.equal(clock.scanWait(), 0);
  });
});
