/**
 * Unit tests for the launch: it plays from the first paint, dissolves on its own,
 * gives way at once to a click or a key, and does not play with reduced motion.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

describe('the launch', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('dissolves on its own, then is gone', () => {
    const app = loadRenderer();
    const { LAUNCH_MS, LAUNCH_LEAVE_MS } = app.run('RendererConstants');
    assert.equal(app.$('launch').hidden, false, 'it is up from the first paint');
    mock.timers.tick(LAUNCH_MS);
    assert.ok(app.$('launch').classList.contains('leaving'));
    mock.timers.tick(LAUNCH_LEAVE_MS);
    assert.equal(app.$('launch').hidden, true);
  });

  it('gives way at once to a key', () => {
    const app = loadRenderer();
    app.window.dispatchWindow('keydown');
    assert.ok(app.$('launch').classList.contains('leaving'));
  });

  it('does not play with reduced motion', () => {
    const app = loadRenderer({ reducedMotion: true });
    assert.equal(app.$('launch').hidden, true);
  });
});
