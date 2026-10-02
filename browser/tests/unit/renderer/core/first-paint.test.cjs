/**
 * Unit tests for the first paint: the theme the main process put in the page's
 * address is on the root before anything paints, and anything else is ignored.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

describe('the first paint', () => {
  it('paints in the theme the address carries', () => {
    const app = loadRenderer({ search: '?theme=dark', answers: { getUiPreferences: new Promise(() => {}) } });
    assert.equal(app.document.documentElement.dataset.theme, 'dark');
  });

  it('ignores a theme it does not draw', () => {
    const app = loadRenderer({ search: '?theme=neon', answers: { getUiPreferences: new Promise(() => {}) } });
    assert.notEqual(app.document.documentElement.dataset.theme, 'neon');
  });
});
