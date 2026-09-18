/**
 * Unit tests for what the driver borrows from the browser package: the
 * analyzer source for a session, and the element-id check that keeps caller
 * values out of evaluated source.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  analyzerSource,
  elementSelector,
  getAnalyzer,
  getApplier,
} from '../../../../src/drivers/cdp/browser-scripts.ts';
import { FIND_ELEMENT_JS, SET_VALUE_JS } from '../../../../src/drivers/cdp/page-scripts.ts';
import { Status } from '../../../../src/platform/http-status.ts';

describe('elementSelector', () => {
  it('turns an analyzer id, number or numeric string, into its attribute selector', () => {
    assert.equal(elementSelector(12), '[data-ac-id="12"]');
    assert.equal(elementSelector('7'), '[data-ac-id="7"]');
  });

  it('refuses anything that is not a whole, non-negative id', () => {
    for (const bad of [-1, 1.5, '1"]', 'abc', null, undefined, { toString: () => '3' }]) {
      assert.throws(() => elementSelector(bad), { status: Status.BAD_REQUEST }, String(bad));
    }
  });
});

describe('analyzer source', () => {
  it('loads the browser package’s analyzer once', () => {
    const analyzer = getAnalyzer();
    assert.equal(typeof analyzer, 'string');
    assert.equal(getAnalyzer(), analyzer);
  });

  it('fills in the session’s tag attribute and turns the analyzer’s own recorder off', () => {
    assert.equal(analyzerSource('a=__OYA_ATTR__;r=__OYA_RECORD__', 'data-x1'), 'a=data-x1;r=false');
  });

  it('loads the persona applier from the browser package', () => {
    assert.equal(typeof getApplier(), 'function');
  });
});

describe('page scripts', () => {
  it('put caller values in as JSON, never raw', () => {
    assert.ok(FIND_ELEMENT_JS('a"b$&').includes(JSON.stringify('a"b$&')), 'replacement patterns are not expanded');
    assert.ok(SET_VALUE_JS('#s', '"); x("').includes('el.value = "\\"); x(\\""'));
  });
});
