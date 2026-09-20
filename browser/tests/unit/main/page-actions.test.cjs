/**
 * Unit tests for the page-actions facade: it keeps the interface main.js
 * uses, and the input facade keeps every name it exported.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { createPageActions } = require('../../../main/page-actions.cjs');
const input = require('../../../main/input.cjs');
const { pageView, pageCtx } = require('../support/page.cjs');

describe('page-actions facade', () => {
  it('returns the three entry points main.js destructures', () => {
    const api = createPageActions(pageCtx(pageView()));
    assert.deepEqual(Object.keys(api).sort(), ['runDevAction', 'runPageAction', 'waitForTabReady']);
    for (const fn of Object.values(api)) assert.equal(typeof fn, 'function');
  });

  it('the entry points work when destructured', async () => {
    const { runDevAction } = createPageActions(pageCtx(null));
    assert.deepEqual(await runDevAction('reload'), { ok: false, error: 'No active tab' });
  });

  it('input keeps its exports', () => {
    assert.deepEqual(Object.keys(input).sort(), [
      'cdpClearField',
      'cdpClick',
      'cdpMouseMove',
      'cdpPressKey',
      'cdpScroll',
      'cdpTypeText',
      'keyDef',
      'sleep',
      'typingDelay',
    ]);
  });
});
