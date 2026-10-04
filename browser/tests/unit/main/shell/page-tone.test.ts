/**
 * Unit tests for judging a page dark or light from its own background colours, so
 * the control shield can dim it the right way.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { pageTone, toneOf, BRIGHTNESS_JS } from '../../../../src/main/shell/page-tone.ts';

describe('page tone', () => {
  it('calls a dim page dark and a bright one light', () => {
    assert.equal(toneOf(0.08), 'dark');
    assert.equal(toneOf(0.95), 'light');
  });

  it('reads the brightness in the page, and says nothing when it cannot be read', async () => {
    const asked = [];
    assert.equal(await pageTone(async (js) => (asked.push(js), 0.1)), 'dark');
    assert.deepEqual(asked, [BRIGHTNESS_JS]);
    assert.equal(await pageTone(async () => Promise.reject(new Error('navigated'))), null);
    assert.equal(await pageTone(async () => 'not a number'), null);
  });
});
