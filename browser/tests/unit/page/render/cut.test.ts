/**
 * Unit tests for src/page/render/cut.ts, the one rule that decides which blocks of a long page a cut
 * keeps: the on-screen elements first, then the text within reach of them, then off-screen form fields,
 * then the blocks nearest the screen; reading order when nothing is on screen. The order is checked through keepOrder and pickBlocks, and the note's words are pinned.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cutNote, keepOrder, pickBlocks } from '../../../../src/page/render/cut.ts';
import type { Block } from '../../../../src/page/render/types.ts';

/** Every block is one unit of room. */
const one = () => 1;

/** A content row, with no id. */
const row = (text: string): Block => ({ kind: 'text', text });

/** A link element: on screen when `on`, otherwise carrying the analyzer's off-screen word. */
const link = (id: number, on: boolean): Block => ({
  id,
  kind: 'link',
  text: `link ${id}`,
  state: on ? '' : 'off-screen',
});

describe('cut rule', () => {
  it('with nothing on screen, keeps the blocks from the top in reading order', () => {
    const blocks = [row('a'), link(1, false), row('b'), row('c'), row('d')];
    const cut = pickBlocks(blocks, 3, one);
    assert.deepEqual(cut.kept, [0, 1, 2]);
    assert.equal(cut.anchored, false);
  });

  it('with nothing on screen, a form field is not moved up: reading order still rules', () => {
    const blocks = [row('a'), { id: 1, kind: 'input:email', text: 'Email', state: 'off-screen' }, row('b')];
    assert.deepEqual(keepOrder(blocks).order, [0, 1, 2]);
  });

  it('the on-screen element comes first, then the nearest row to it', () => {
    const blocks = [link(1, false), row('far'), link(2, false), link(3, true), row('beside')];
    const cut = pickBlocks(blocks, 2, one);
    assert.deepEqual(cut.kept, [3, 4]);
    assert.equal(cut.anchored, true);
  });

  it('the on-screen elements come before any content row, even one right beside them', () => {
    const blocks = [link(1, false), row('beside'), link(2, true)];
    assert.deepEqual(keepOrder(blocks).order, [2, 1, 0]);
  });

  it('the only on-screen element is first, and the rows around it come after', () => {
    const blocks = [row('a'), row('b'), link(3, true), row('c')];
    assert.deepEqual(keepOrder(blocks).order.slice(0, 1), [2]);
  });

  it('orders by distance to the screen, a row below the element before one above it at the same distance', () => {
    const blocks = [row('above'), link(1, true), row('below'), row('farther')];
    assert.deepEqual(keepOrder(blocks).order, [1, 2, 0, 3]);
  });

  it('an off-screen form field is kept before off-screen elements and far rows; near text comes before it', () => {
    const blocks = [
      link(1, false),
      row('far'),
      { id: 2, kind: 'input:email', text: 'Email', state: 'off-screen' },
      row('near'),
      link(4, false),
      link(5, true),
    ];
    assert.deepEqual(pickBlocks(blocks, 3, one).kept, [2, 3, 5]);
  });

  it('near text is kept before off-screen fields, and text beyond the reach comes after them', () => {
    const blocks = [
      row('a'),
      row('b'),
      row('c'),
      row('d'),
      { id: 2, kind: 'input:email', text: 'Email', state: 'off-screen' },
      link(3, true),
    ];
    assert.deepEqual(keepOrder(blocks).order, [5, 3, 2, 4, 1, 0]);
  });

  it('on a distance tie the block below its anchor comes first, then the one above', () => {
    const blocks = [row('above'), link(1, false), link(2, true), link(3, false), row('below')];
    assert.deepEqual(keepOrder(blocks).order, [2, 4, 0, 3, 1]);
  });

  it('an anchored cut skips a block that does not fit and keeps the next one that does', () => {
    const blocks = [
      { id: 1, kind: 'link', text: 'a'.repeat(8), state: '' },
      { id: 2, kind: 'link', text: 'b', state: '' },
    ];
    const size = (block: Block) => (block.text ?? '').length;
    assert.deepEqual(pickBlocks(blocks, 5, size).kept, [1]);
    assert.deepEqual(pickBlocks(blocks, 9, size).kept, [0, 1]);
  });

  it('an unanchored cut stops at the first block that does not fit, keeping the top as before', () => {
    const blocks = [row('a'.repeat(8)), row('b')];
    const size = (block: Block) => (block.text ?? '').length;
    assert.deepEqual(pickBlocks(blocks, 5, size).kept, []);
    assert.deepEqual(pickBlocks(blocks, 9, size).kept, [0, 1]);
  });

  it('the on-screen element comes first, then the text within reach, then the off-screen field', () => {
    const blocks = [row('near'), { id: 2, kind: 'input:email', text: 'Email', state: 'off-screen' }, link(3, true)];
    assert.deepEqual(keepOrder(blocks).order, [2, 0, 1]);
  });

  it('is anchored only when an on-screen element exists', () => {
    assert.equal(pickBlocks([row('a'), link(1, false)], 5, one).anchored, false);
    assert.equal(pickBlocks([row('a'), link(1, true)], 5, one).anchored, true);
  });
});

describe('cutNote', () => {
  it('says how many blocks a cut kept, and whether they were taken from the screen or the top', () => {
    assert.equal(
      cutNote(3, 9, true),
      'showing 3 of 9 blocks around what is on screen; scroll and analyze again for the rest',
    );
    assert.equal(cutNote(3, 9, false), 'showing 3 of 9 blocks from the top; scroll and analyze again for the rest');
  });
});
