/**
 * Unit tests for the tab strip's arithmetic: tab widths, the size a tab draws
 * at, where a dragged tab lands, how its neighbours slide, how fast the strip
 * scrolls under it, where the hover card sits, and keyboard steps.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import * as M from '../../../../../../src/renderer/features/tabs/model/tab-math.ts';

/** Four 100px tabs side by side. */
const SLOTS = [0, 100, 200, 300].map((left) => ({ left, width: 100 }));

describe('tab math', () => {
  it('shares the room evenly, never wider than 240px', () => {
    assert.equal(M.tabWidth(1000, 1), 240);
    assert.equal(M.tabWidth(1000, 6), 166);
    assert.equal(M.tabWidth(1000, 0), 240);
  });

  it('never narrower than an icon, so past that the strip scrolls', () => {
    assert.equal(M.tabWidth(1000, 25), 44);
    assert.equal(M.tabWidth(-50, 3), 44, 'a hidden strip has no room at all');
  });

  it('drops the close button on narrow tabs, then the title', () => {
    assert.equal(M.tabSize(240), 'normal');
    assert.equal(M.tabSize(80), 'small');
    assert.equal(M.tabSize(44), 'tiny');
  });

  it('lands a dragged tab past every tab whose middle its leading edge crossed', () => {
    assert.equal(M.dropIndex(SLOTS, 0, 0), 0);
    assert.equal(M.dropIndex(SLOTS, 0, 49), 0, 'not yet past the next middle');
    assert.equal(M.dropIndex(SLOTS, 0, 51), 1);
    assert.equal(M.dropIndex(SLOTS, 0, 300), 3);
    assert.equal(M.dropIndex(SLOTS, 3, -101), 2);
    assert.equal(M.dropIndex(SLOTS, 3, -151), 1);
    assert.equal(M.dropIndex(SLOTS, 3, -300), 0);
  });

  it('slides the tabs between the start and the landing place by one tab width, the other way', () => {
    assert.deepEqual(M.shifts(SLOTS, 0, 2), [0, -100, -100, 0]);
    assert.deepEqual(M.shifts(SLOTS, 3, 1), [0, 100, 100, 0]);
    assert.deepEqual(M.shifts(SLOTS, 1, 1), [0, 0, 0, 0]);
  });

  it('keeps a dragged tab within the strip', () => {
    assert.equal(M.clamp(SLOTS, 1, -500), -100);
    assert.equal(M.clamp(SLOTS, 1, 500), 200);
    assert.equal(M.clamp(SLOTS, 1, 30), 30);
  });

  it('lands a moved tab where the others closed up behind it', () => {
    assert.equal(M.landingLeft(SLOTS, 0, 2), 200);
    assert.equal(M.landingLeft(SLOTS, 3, 1), 100);
    const mixed = [
      { left: 0, width: 50 },
      { left: 50, width: 100 },
    ];
    assert.equal(M.landingLeft(mixed, 0, 1), 100, 'a narrow tab lands at the right of the wide one it passed');
  });

  it('scrolls the strip only near its edges, faster the closer the pointer is', () => {
    assert.equal(M.edgeSpeed(500, 0, 1000), 0);
    assert.ok(M.edgeSpeed(995, 0, 1000) > M.edgeSpeed(980, 0, 1000));
    assert.ok(M.edgeSpeed(980, 0, 1000) > 0);
    assert.ok(M.edgeSpeed(5, 0, 1000) < 0);
    assert.equal(M.edgeSpeed(-20, 0, 1000), -14, 'at most the top speed');
  });

  it('keeps the hover card inside the window', () => {
    assert.equal(M.cardLeft(100, 1000, 200), 100);
    assert.equal(M.cardLeft(900, 1000, 200), 800);
    assert.equal(M.cardLeft(-10, 1000, 200), 0);
  });
});
