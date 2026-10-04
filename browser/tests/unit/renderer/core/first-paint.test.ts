/**
 * Unit tests for the first paint (src/renderer/public/first-paint.js): the
 * theme the main process put in the page's address is on the root before
 * anything paints, and anything else is ignored.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

/** The script's source. */
const SOURCE = fs.readFileSync(new URL('../../../../src/renderer/public/first-paint.js', import.meta.url), 'utf8');

/** The root's dataset after the script runs in a page whose address has `search`. */
function paint(search: string): Record<string, string> {
  const dataset: Record<string, string> = {};
  vm.runInNewContext(SOURCE, { location: { search }, document: { documentElement: { dataset } }, URLSearchParams });
  return dataset;
}

describe('the first paint', () => {
  it('paints in the theme the address carries', () => {
    assert.equal(paint('?theme=dark').theme, 'dark');
    assert.equal(paint('?theme=light').theme, 'light');
  });

  it('ignores a theme it does not draw, and an address without one', () => {
    assert.equal(paint('?theme=neon').theme, undefined);
    assert.equal(paint('').theme, undefined);
  });
});
