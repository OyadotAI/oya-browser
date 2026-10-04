/**
 * Unit tests for the tab strip's arithmetic: tab widths, the size a tab draws
 * at, where a dragged tab lands, how its neighbours slide, and how fast the
 * strip scrolls under it.
 */
const { describe, it, before } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

/** Four 100px tabs side by side. */
const SLOTS = [0, 100, 200, 300].map((left) => ({ left, width: 100 }));

describe('TabMath', () => {
  let TabMath;
  before(() => (TabMath = loadRenderer().run('TabMath')));

  it('shares the room evenly, never wider than 240px', () => {
    assert.equal(TabMath.width(1000, 1), 240);
    assert.equal(TabMath.width(1000, 6), 166);
    assert.equal(TabMath.width(1000, 0), 240);
  });

  it('never narrower than an icon, so past that the strip scrolls', () => {
    assert.equal(TabMath.width(1000, 25), 44);
    assert.equal(TabMath.width(-50, 3), 44, 'a hidden strip has no room at all');
  });

  it('drops the close button on narrow tabs, then the title', () => {
    assert.equal(TabMath.size(240), 'normal');
    assert.equal(TabMath.size(80), 'small');
    assert.equal(TabMath.size(44), 'tiny');
  });

  it('lands a dragged tab past every tab whose middle its leading edge crossed', () => {
    assert.equal(TabMath.dropIndex(SLOTS, 0, 0), 0);
    assert.equal(TabMath.dropIndex(SLOTS, 0, 49), 0, 'not yet past the next middle');
    assert.equal(TabMath.dropIndex(SLOTS, 0, 51), 1);
    assert.equal(TabMath.dropIndex(SLOTS, 0, 300), 3);
    assert.equal(TabMath.dropIndex(SLOTS, 3, -101), 2);
    assert.equal(TabMath.dropIndex(SLOTS, 3, -151), 1);
    assert.equal(TabMath.dropIndex(SLOTS, 3, -300), 0);
  });

  it('slides the tabs between the start and the landing place by one tab width, the other way', () => {
    assert.deepEqual(TabMath.shifts(SLOTS, 0, 2), [0, -100, -100, 0]);
    assert.deepEqual(TabMath.shifts(SLOTS, 3, 1), [0, 100, 100, 0]);
    assert.deepEqual(TabMath.shifts(SLOTS, 1, 1), [0, 0, 0, 0]);
  });

  it('keeps a dragged tab within the strip', () => {
    assert.equal(TabMath.clamp(SLOTS, 1, -500), -100);
    assert.equal(TabMath.clamp(SLOTS, 1, 500), 200);
    assert.equal(TabMath.clamp(SLOTS, 1, 30), 30);
  });

  it('scrolls the strip only near its edges, faster the closer the pointer is', () => {
    assert.equal(TabMath.edgeSpeed(500, 0, 1000), 0);
    assert.ok(TabMath.edgeSpeed(995, 0, 1000) > TabMath.edgeSpeed(980, 0, 1000));
    assert.ok(TabMath.edgeSpeed(980, 0, 1000) > 0);
    assert.ok(TabMath.edgeSpeed(5, 0, 1000) < 0);
    assert.equal(TabMath.edgeSpeed(-20, 0, 1000), -14, 'at most the top speed');
  });
});
