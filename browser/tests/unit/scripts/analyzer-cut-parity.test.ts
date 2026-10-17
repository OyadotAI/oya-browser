/**
 * Pins the analyzer's copy of the cap to its source in src/page: its note's wording, and its
 * capOrder keeps the same rows in the same order as keepOrder in cut.ts. analyzer.js is plain JS
 * that pages can see, so it cannot import these; a drift must fail here instead of on a real page.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { cutNote, keepOrder } from '../../../src/page/render/cut.ts';
import type { Block } from '../../../src/page/render/types.ts';

const analyzer = readFileSync(new URL('../../../scripts/analyzer.js', import.meta.url), 'utf8');

/** The cap's functions as they are written in the analyzer: from the declaration to the closing brace. */
function analyzerFunction(name: string): string {
  const start = analyzer.indexOf(`  function ${name}(`);
  assert.ok(start >= 0, `analyzer.js declares ${name}`);
  return analyzer.slice(start, analyzer.indexOf('\n  }\n', start) + '\n  }\n'.length);
}

/** What the analyzer's cap returns: the rows it keeps, and whether an on-screen row anchored it. */
interface CapResult {
  /** The kept rows, in reading order. */
  rows: Block[];
  /** Whether a row on screen anchored the cap. */
  anchored: boolean;
}

/** The two cap functions of the analyzer, as the test calls them. */
interface AnalyzerCap {
  /** The positions of the rows, most worth keeping first. */
  capOrder: (rows: Block[]) => number[];
  /** The rows the cap keeps. */
  capRows: (rows: Block[]) => CapResult;
}

/** The analyzer's capOrder and capRows, run as written in the file, with a MAX_BLOCKS of `maxBlocks`. */
function analyzerCap(maxBlocks: number): AnalyzerCap {
  const code = ['screenDistances', 'capOrder', 'capRows'].map(analyzerFunction).join('\n');
  return runInNewContext(`${code}\n({ capOrder, capRows })`, { MAX_BLOCKS: maxBlocks }) as AnalyzerCap;
}

/** Twelve rows, index 0 to 11. The rows listed in `onScreen` are on screen; row 6 is an email field. */
function fixture(onScreen: number[]): Block[] {
  const rows: Block[] = [
    { kind: 'text', text: 'Text 0' },
    { id: 1, kind: 'link', text: 'Link 1', state: 'off-screen' },
    { kind: 'text', text: 'Text 2' },
    { id: 3, kind: 'button', text: 'Button 3', state: 'off-screen' },
    { kind: 'text', text: 'Text 4' },
    { kind: 'text', text: 'Text 5' },
    { id: 6, kind: 'input:email', text: 'Email 6', state: 'invalid: off-screen; off-screen' },
    { kind: 'text', text: 'Text 7' },
    { id: 8, kind: 'button', text: 'Button 8', state: 'off-screen' },
    { kind: 'text', text: 'Text 9' },
    { id: 10, kind: 'link', text: 'Link 10', state: 'off-screen' },
    { kind: 'text', text: 'Text 11' },
  ];
  return rows.map((row, index) => ({ ...row, index, ...(onScreen.includes(index) ? { screen: true } : {}) }));
}

describe('analyzer cut parity with src/page', () => {
  it('words its truncated note the way cutNote does', () => {
    for (const phrase of ['around what is on screen', 'from the top', 'scroll and analyze again for the rest'])
      assert.ok(analyzer.includes(phrase), phrase);
  });

  it('builds its truncated note from the same template as cutNote, with the anchored and top wording', () => {
    const template =
      "showing ${rows.length} of ${blocks.length} blocks ${anchored ? 'around what is on screen' : 'from the top'}; scroll and analyze again for the rest";
    assert.ok(analyzer.includes(template), 'the analyzer note template is unchanged');
    assert.equal(
      cutNote(1, 2, true),
      'showing 1 of 2 blocks around what is on screen; scroll and analyze again for the rest',
    );
    assert.equal(cutNote(1, 2, false), 'showing 1 of 2 blocks from the top; scroll and analyze again for the rest');
  });

  it('keeps the same rows in the same order as keepOrder when rows on screen anchor the cap', () => {
    const rows = fixture([5, 6]);
    const { capOrder, capRows } = analyzerCap(5);
    const expected = [5, 6, 7, 4, 8, 3, 9, 2, 10, 1, 11, 0];
    assert.deepEqual(capOrder(rows), expected);
    assert.deepEqual(keepOrder(rows).order, expected);
    assert.equal(keepOrder(rows).anchored, true);
    assert.deepEqual(
      capRows(rows).rows.map((row) => row.index),
      [4, 5, 6, 7, 8],
    );
    assert.equal(capRows(rows).anchored, true);
  });

  it('keeps reading order and the first rows when nothing is on screen', () => {
    const rows = fixture([]);
    const { capOrder, capRows } = analyzerCap(5);
    const reading = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    assert.deepEqual(capOrder(rows), reading);
    assert.deepEqual(keepOrder(rows).order, reading);
    assert.equal(keepOrder(rows).anchored, false);
    assert.deepEqual(
      capRows(rows).rows.map((row) => row.index),
      [0, 1, 2, 3, 4],
    );
    assert.equal(capRows(rows).anchored, false);
  });
});
