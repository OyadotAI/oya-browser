/**
 * Unit tests for src/page/render/cut.ts, the one rule that decides which blocks of a long page a cut
 * keeps: the rows the analyzer marked on screen first, then the rows nearest them by page position;
 * reading order when nothing is on screen. The order is checked through keepOrder and pickBlocks, and the note's words are pinned.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cutNote, keepOrder, pickBlocks } from '../../../../src/page/render/cut.ts';
import type { Block } from '../../../../src/page/render/types.ts';

/** Every block is one unit of room. */
const one = () => 1;

/** The size of a block's text, so a test can set each block's size with its text. */
/** A block of a given size: sizes ride on the block, since a long text is clipped before it is sized. */
const sized = (sz: number, extra: Partial<Block> = {}): Block => ({ kind: 'text', text: '', sz, ...extra });

/** The size a sized block carries. */
const bySz = (block: Block) => Number(block.sz);

/** A content row, with no id. */
const row = (text: string): Block => ({ kind: 'text', text });

/** A link element: on screen when `on` (the analyzer's screen mark), otherwise carrying the off-screen word. */
const link = (id: number, on: boolean): Block => ({
  id,
  kind: 'link',
  text: `link ${id}`,
  state: on ? '' : 'off-screen',
  screen: on,
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

  it('an off-screen form field gets no priority: it is ranked by distance like the rows around it', () => {
    // The field tier is gone: the field at distance 3 is dropped before the nearer row at distance 2.
    const blocks = [
      link(1, false),
      row('far'),
      { id: 2, kind: 'input:email', text: 'Email', state: 'off-screen' },
      row('near'),
      link(4, false),
      link(5, true),
    ];
    assert.deepEqual(pickBlocks(blocks, 3, one).kept, [3, 4, 5]);
  });

  it('ranks by distance alone: the nearest row comes first, whatever its kind', () => {
    const blocks = [
      row('a'),
      row('b'),
      row('c'),
      row('d'),
      { id: 2, kind: 'input:email', text: 'Email', state: 'off-screen' },
      link(3, true),
    ];
    assert.deepEqual(keepOrder(blocks).order, [5, 4, 3, 2, 1, 0]);
  });

  it('on a distance tie the block below its anchor comes first, then the one above', () => {
    const blocks = [row('above'), link(1, false), link(2, true), link(3, false), row('below')];
    assert.deepEqual(keepOrder(blocks).order, [2, 3, 1, 4, 0]);
  });

  it('an anchored cut skips a block that does not fit and keeps a smaller one after it', () => {
    const blocks = [sized(8, { id: 1, kind: 'link', screen: true }), sized(1, { id: 2, kind: 'link', screen: true })];
    assert.deepEqual(pickBlocks(blocks, 5, bySz).kept, [1]);
    assert.deepEqual(pickBlocks(blocks, 9, bySz).kept, [0, 1]);
  });

  it('an unanchored cut stops at the first block that does not fit, keeping the top as before', () => {
    const blocks = [sized(8), sized(1)];
    assert.deepEqual(pickBlocks(blocks, 5, bySz).kept, []);
    assert.deepEqual(pickBlocks(blocks, 9, bySz).kept, [0, 1]);
  });

  it('the on-screen element comes first, then the rows by distance, the field and the near row alike', () => {
    const blocks = [row('near'), { id: 2, kind: 'input:email', text: 'Email', state: 'off-screen' }, link(3, true)];
    assert.deepEqual(keepOrder(blocks).order, [2, 1, 0]);
  });

  it('is anchored only when an on-screen element exists', () => {
    assert.equal(pickBlocks([row('a'), link(1, false)], 5, one).anchored, false);
    assert.equal(pickBlocks([row('a'), link(1, true)], 5, one).anchored, true);
  });

  it('E1 anchors on the screen mark and orders by position when there is no index', () => {
    const blocks = [row('0'), row('1'), { ...row('2'), screen: true }, row('3'), row('4')];
    const { order, anchored } = keepOrder(blocks);
    assert.deepEqual(order, [2, 3, 1, 4, 0]);
    assert.equal(anchored, true);
  });

  it('E2 measures distance in the analyzer index, so a capped gap counts as the rows it left out', () => {
    const blocks: Block[] = [0, 1, 2, 10, 11].map((index) => ({ kind: 'text', text: `${index}`, index }));
    blocks[3].screen = true;
    assert.deepEqual(keepOrder(blocks).order, [3, 4, 2, 1, 0]);
  });

  it('E3 with no screen mark, the order is reading order and not anchored, even with an element', () => {
    const blocks: Block[] = [row('0'), { id: 1, kind: 'link', text: 'x', state: '' }, row('2')];
    assert.deepEqual(keepOrder(blocks), { order: [0, 1, 2], anchored: false });
  });

  it('E4 an element anchors by its screen mark, never by its state words', () => {
    const marked: Block[] = [{ id: 1, kind: 'input:text', state: 'invalid: x; off-screen', screen: true }];
    const unmarked: Block[] = [{ id: 1, kind: 'link', state: '' }];
    assert.equal(keepOrder(marked).anchored, true);
    assert.equal(keepOrder(unmarked).anchored, false);
  });

  it('E5 an anchored pick skips a block that does not fit the room left and takes the next one that does', () => {
    const blocks: Block[] = [sized(1, { id: 1, kind: 'link', screen: true }), sized(2), sized(1)];
    assert.deepEqual(pickBlocks(blocks, 2, bySz).kept, [0, 2]);
  });

  it('E6 a clipped block near the screen does not leave the rest of the room unused', () => {
    const blocks: Block[] = [sized(1, { id: 1, kind: 'link', screen: true }), sized(5), sized(1), sized(1)];
    assert.deepEqual(pickBlocks(blocks, 3, bySz).kept, [0, 2, 3]);
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
