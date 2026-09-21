/**
 * Unit tests for the control shield's page: the scan while an agent reads the
 * page, the outlines of what it found, and the companion's words.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../support/renderer-harness.cjs');

/** Steps of time to let every chained timer of one show run. */
const STEPS = 10;
/** One step, in milliseconds. */
const STEP_MS = 1000;

describe('the control shield page', () => {
  let app;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    app = loadRenderer({ page: 'control-shield.html' });
  });
  afterEach(() => mock.timers.reset());

  /** Lets time pass step by step, so timers set by timers run too. */
  const drain = () => {
    for (let i = 0; i < STEPS; i++) mock.timers.tick(STEP_MS);
  };
  /** The bubble's words. */
  const said = () => app.$('bubble-text').textContent;
  /** The outlines on the stage. */
  const boxes = () => app.$('stage').children;

  it('scans and says so while the analysis runs', () => {
    app.window.oyaShield({ phase: 'scan' });
    assert.ok(app.document.body.classList.contains('scanning'));
    assert.equal(said(), 'Reading the page');
    assert.ok(app.$('companion').classList.contains('talking'));
  });

  it('outlines what was found top to bottom, numbered and coloured by kind, after the scan', () => {
    app.window.oyaShield({ phase: 'scan' });
    const found = [
      { id: 2, type: 'link', x: 10, y: 300, w: 50, h: 20 },
      { id: 1, type: 'button', x: 10, y: 20, w: 80, h: 30 },
    ];
    app.window.oyaShield({ phase: 'found', boxes: found });
    assert.equal(boxes().length, 0, 'the scan runs its minimum first');
    mock.timers.tick(app.run('RendererConstants.SHIELD_MIN_SCAN_MS'));
    assert.deepEqual(
      [...boxes()].map((b) => b.dataset.id),
      ['1', '2'],
    );
    assert.equal(boxes()[0].style['--c'], '#39ed35');
    assert.equal(boxes()[0].style.top, '20px');
    assert.equal(said(), 'Found 2 elements');
    assert.ok(!app.document.body.classList.contains('scanning'));
  });

  it('lights each outline up as the reveal reaches it, top first', () => {
    const reveal = app.run('RendererConstants.SHIELD_REVEAL_MS');
    const found = [
      { id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 },
      { id: 2, type: 'link', x: 0, y: 400, w: 5, h: 5 },
      { id: 3, type: 'link', x: 0, y: 5000, w: 5, h: 5 },
    ];
    app.window.oyaShield({ phase: 'found', boxes: found });
    mock.timers.tick(0);
    assert.deepEqual(
      [...boxes()].map((b) => b.style['--d']),
      ['0ms', `${reveal / 2}ms`, `${reveal}ms`],
    );
    assert.ok(app.document.body.classList.contains('revealing'));
    assert.ok(app.$('companion').classList.contains('found'));
  });

  it('clears the outlines and goes quiet after a while', () => {
    app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 }] });
    drain();
    assert.equal(boxes().length, 0);
    assert.ok(!app.$('companion').classList.contains('talking'));
  });

  it('a new scan cancels the last one’s outlines', () => {
    app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 }] });
    app.window.oyaShield({ phase: 'scan' });
    drain();
    assert.equal(boxes().length, 0);
    assert.equal(said(), 'Reading the page');
  });

  it('says when there is nothing to click, and ignores an unknown update', () => {
    app.window.oyaShield({ phase: 'found', boxes: [] });
    mock.timers.tick(0);
    assert.equal(said(), 'Nothing to interact with here');
    app.window.oyaShield({ phase: 'constructor' });
    assert.equal(said(), 'Nothing to interact with here');
  });
});
