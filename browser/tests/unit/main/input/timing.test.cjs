/**
 * Unit tests for input timing: pauses land inside their ranges, and the gap
 * after a keystroke grows at word boundaries, special characters and the
 * occasional hesitation.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { sleep, jitter, typingDelay } = require('../../../../main/input/timing.cjs');
const c = require('../../../../main/input/constants.cjs');

describe('input timing', () => {
  afterEach(() => mock.restoreAll());

  it('jitter spans base to base + spread', () => {
    mock.method(Math, 'random', () => 0);
    assert.equal(jitter({ base: 10, spread: 20 }), 10);
    mock.method(Math, 'random', () => 0.5);
    assert.equal(jitter({ base: 10, spread: 20 }), 20);
  });

  it('a letter after a letter gets only the base gap', () => {
    mock.method(Math, 'random', () => 0.5);
    assert.equal(typingDelay('a', 'b'), jitter(c.TYPE_BASE));
  });

  it('a word boundary adds a pause', () => {
    mock.method(Math, 'random', () => 0.5);
    assert.equal(typingDelay('a', ' '), jitter(c.TYPE_BASE) + jitter(c.WORD_PAUSE));
  });

  it('a special character adds a pause', () => {
    mock.method(Math, 'random', () => 0.5);
    assert.equal(typingDelay('%', 'a'), jitter(c.TYPE_BASE) + jitter(c.SPECIAL_PAUSE));
  });

  it('a rare roll adds a hesitation', () => {
    mock.method(Math, 'random', () => 0);
    assert.equal(typingDelay('a', 'b'), c.TYPE_BASE.base + c.HESITATION.base);
  });

  it('sleep resolves after its delay', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    let done = false;
    const waiting = sleep(100).then(() => (done = true));
    mock.timers.tick(99);
    await Promise.resolve();
    assert.equal(done, false);
    mock.timers.tick(1);
    await waiting;
    assert.equal(done, true);
    mock.timers.reset();
  });
});
