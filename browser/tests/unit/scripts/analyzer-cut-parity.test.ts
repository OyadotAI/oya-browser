/**
 * Pins the analyzer's copies of the cut rules to their source in src/page: the reach,
 * the field kinds and the note's wording. analyzer.js is plain JS that pages can see,
 * so it cannot import these; a drift must fail here instead of on a real page.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PAGE_RENDER } from '../../../src/page/constants.ts';
import { FIELD_TYPES, cutNote } from '../../../src/page/render/cut.ts';

const analyzer = readFileSync(new URL('../../../scripts/analyzer.js', import.meta.url), 'utf8');

/** The quoted kinds in the analyzer's FIELD_KINDS array, in the order it lists them. */
function analyzerFieldKinds(source: string): string[] {
  const list = /const FIELD_KINDS = \[([^\]]*)\];/.exec(source);
  return [...(list?.[1] ?? '').matchAll(/'([^']+)'/g)].map((match) => match[1]);
}

describe('analyzer cut parity with src/page', () => {
  it('uses the same SCREEN_TEXT_REACH as the page renderers', () => {
    assert.ok(analyzer.includes(`const SCREEN_TEXT_REACH = ${PAGE_RENDER.SCREEN_TEXT_REACH};`));
  });

  it('lists the same field kinds as FIELD_TYPES, in the same order', () => {
    assert.deepEqual(analyzerFieldKinds(analyzer), [...FIELD_TYPES]);
  });

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
});
