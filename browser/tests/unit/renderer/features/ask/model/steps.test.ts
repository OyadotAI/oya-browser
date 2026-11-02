/**
 * Unit tests for a finished run's summary and its token figure: the total is
 * written short after the time, a missing or zero count is left out rather
 * than shown as zero, and the tooltip gives the exact split.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { runSummary, usageTitle } from '../../../../../../src/renderer/features/ask/model/steps.ts';

describe('runSummary', () => {
  it('the summary adds compact tokens after the time', () => {
    assert.equal(runSummary(14, 48, { input: 150000, output: 62000 }), '14 steps · 48s · 212K tokens');
    assert.equal(runSummary(3, 12, { input: 1200000, output: 200000 }), '3 steps · 12s · 1.4M tokens');
    assert.equal(runSummary(1, 4, { input: 800, output: 40 }), '1 step · 4s · 840 tokens');
  });

  it('missing or zero usage leaves the figure out', () => {
    assert.equal(runSummary(3, 12), '3 steps · 12s');
    assert.equal(runSummary(3, 12, { input: 0, output: 0 }), '3 steps · 12s');
    assert.equal(runSummary(3, 12, {}), '3 steps · 12s');
  });

  it('a count that is not a positive number is treated as none', () => {
    const odd = { input: Number.NaN, output: -5 } as const;
    assert.equal(runSummary(3, 12, odd), '3 steps · 12s');
    assert.equal(runSummary(3, 12, { input: 'lots' as unknown as number, output: 10 }), '3 steps · 12s · 10 tokens');
  });

  it('one token is singular', () => {
    assert.equal(runSummary(1, 1, { input: 1, output: 0 }), '1 step · 1s · 1 token');
  });
});

describe('usageTitle', () => {
  it('the title gives the exact split with separators', () => {
    assert.equal(usageTitle({ input: 148210, output: 63904 }), '148,210 input · 63,904 output tokens');
  });

  it('no usage gives no title', () => {
    assert.equal(usageTitle(), '');
    assert.equal(usageTitle({ input: 0, output: 0 }), '');
  });
});
