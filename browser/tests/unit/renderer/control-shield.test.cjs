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
  /** Lets the show start, then the beam finish its pass, so every outline has locked on. */
  const reveal = () => {
    mock.timers.tick(0);
    mock.timers.tick(app.run('RendererConstants.SHIELD_REVEAL_MS'));
  };

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
    reveal();
    assert.deepEqual(
      [...boxes()].map((b) => b.dataset.id),
      ['1', '2'],
    );
    assert.equal(boxes()[0].style['--c'], '#39ed35');
    assert.equal(boxes()[0].style.transform, 'translate3d(10px, 20px, 0)');
    assert.equal(boxes()[0].textContent, '1');
    assert.ok(!app.document.body.classList.contains('scanning'));
  });

  it('adds each outline as the beam reaches it, top first', () => {
    const reveal = app.run('RendererConstants.SHIELD_REVEAL_MS');
    const found = [
      { id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 },
      { id: 2, type: 'link', x: 0, y: 400, w: 5, h: 5 },
      { id: 3, type: 'link', x: 0, y: 5000, w: 5, h: 5 },
    ];
    app.window.oyaShield({ phase: 'found', boxes: found });
    const shown = () => [...boxes()].map((b) => b.dataset.id);
    mock.timers.tick(0);
    assert.deepEqual(shown(), ['1']);
    assert.ok(app.document.body.classList.contains('revealing'));
    assert.ok(app.$('companion').classList.contains('found'));
    mock.timers.tick(reveal / 2);
    assert.deepEqual(shown(), ['1', '2']);
    mock.timers.tick(reveal / 2);
    assert.deepEqual(shown(), ['1', '2', '3'], 'one below the page is reached as the pass ends');
  });

  it('clears the outlines and goes quiet after a while', () => {
    app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 }] });
    drain();
    assert.equal(boxes().length, 0);
    assert.ok(!app.$('companion').classList.contains('talking'));
  });

  it('counts the elements up as the beam reaches each one', () => {
    const found = [
      { id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 },
      { id: 2, type: 'link', x: 0, y: 400, w: 5, h: 5 },
    ];
    app.window.oyaShield({ phase: 'found', boxes: found });
    mock.timers.tick(0);
    assert.equal(said(), 'Found 1 element', 'the top one is reached at once');
    mock.timers.tick(app.run('RendererConstants.SHIELD_REVEAL_MS'));
    assert.equal(said(), 'Found 2 elements');
  });

  it('starts each outline’s brackets just outside its element, so they lock on', () => {
    const reach = app.run('RendererConstants.SHIELD_LOCK_REACH_PX');
    app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'input', x: 0, y: 0, w: reach * 10, h: reach }] });
    mock.timers.tick(0);
    assert.equal(boxes()[0].style['--sx'], '1.1');
    assert.equal(boxes()[0].style['--sy'], '2');
  });

  describe('sparks into the orb', () => {
    /** Lays the orb out at 1000,700, 50 across, so its middle is 1025,725. */
    beforeEach(() => {
      app.document.querySelector('.orb').getBoundingClientRect = () => ({
        left: 1000,
        top: 700,
        width: 50,
        height: 50,
      });
    });
    /** The sparks in flight. */
    const sparks = () => [...app.$('sparks').children];

    it('sends a spark from the middle of each element towards the middle of the orb, after the lock', () => {
      const lag = app.run('RendererConstants.SHIELD_SPARK_LAG_MS');
      app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'input', x: 10, y: 0, w: 100, h: 40 }] });
      mock.timers.tick(0);
      const [spark] = sparks();
      assert.deepEqual(
        ['--fx', '--fy', '--dx', '--dy', '--d'].map((name) => spark.style[name]),
        ['60px', '20px', '965px', '705px', `${lag}ms`],
      );
      assert.equal(spark.style['--c'], '#6cb4ff');
    });

    it('sends at most SHIELD_SPARKS_MAX, spread over the page, and clears them with the outlines', () => {
      const max = app.run('RendererConstants.SHIELD_SPARKS_MAX');
      const many = Array.from({ length: max * 3 }, (_, i) => ({ id: i, type: 'link', x: 0, y: i, w: 5, h: 5 }));
      app.window.oyaShield({ phase: 'found', boxes: many });
      reveal();
      assert.equal(sparks().length, max);
      drain();
      assert.equal(sparks().length, 0);
    });
  });

  it('glides each outline to where its element moved, without restarting the show', () => {
    app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'link', x: 0, y: 40, w: 50, h: 20 }] });
    reveal();
    const outline = boxes()[0];
    app.window.oyaShield({ phase: 'move', boxes: [{ id: 1, type: 'link', x: 0, y: 10, w: 60, h: 20 }] });
    assert.equal(boxes()[0], outline);
    assert.equal(outline.style.transform, 'translate3d(0px, 10px, 0)');
    assert.equal(outline.style.width, '60px');
    drain();
    assert.equal(boxes().length, 0, 'the hold and fade still run');
  });

  it('fades an outline whose element is gone, and brings it back if it returns', () => {
    const one = { id: 1, type: 'link', x: 0, y: 0, w: 5, h: 5 };
    app.window.oyaShield({ phase: 'found', boxes: [one, { id: 2, type: 'link', x: 0, y: 9, w: 5, h: 5 }] });
    reveal();
    app.window.oyaShield({ phase: 'move', boxes: [one] });
    assert.deepEqual(
      [...boxes()].map((b) => b.classList.contains('gone')),
      [false, true],
    );
    app.window.oyaShield({ phase: 'move', boxes: [one, { id: 2, type: 'link', x: 0, y: 9, w: 5, h: 5 }] });
    assert.ok(!boxes()[1].classList.contains('gone'));
  });

  it('outlines where the elements moved to while the scan was still running', () => {
    app.window.oyaShield({ phase: 'scan' });
    app.window.oyaShield({ phase: 'found', boxes: [{ id: 1, type: 'link', x: 0, y: 300, w: 5, h: 5 }] });
    app.window.oyaShield({ phase: 'move', boxes: [{ id: 1, type: 'link', x: 0, y: 100, w: 5, h: 5 }] });
    mock.timers.tick(app.run('RendererConstants.SHIELD_MIN_SCAN_MS'));
    reveal();
    assert.equal(boxes()[0].style.transform, 'translate3d(0px, 100px, 0)');
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
