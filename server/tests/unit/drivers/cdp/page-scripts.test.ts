/** Explicit CSS and opaque analyzer references retain distinct resolution paths. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runInNewContext } from 'node:vm';
import { INPUT_TYPE_JS, SELECT_CONTENTS_JS, FIND_ELEMENT_JS } from '../../../../src/drivers/cdp/page-scripts.ts';

test('CSS selectors remain usable when an ID-only analyzer is installed', () => {
  let selected = false;
  const node = {
    tagName: 'INPUT',
    type: 'text',
    select: () => {
      selected = true;
    },
  };
  const context = {
    window: {
      __acFindElement: () => {
        throw Error('CSS must not be sent as an analyzer ID');
      },
    },
    document: {
      querySelector: (selector) => {
        assert.equal(selector, '#search');
        return node;
      },
    },
  };
  assert.equal(runInNewContext(INPUT_TYPE_JS('#search'), context), 'text');
  runInNewContext(SELECT_CONTENTS_JS('#search'), context);
  assert.equal(selected, true);
});

test('a missing analyzer reference never falls back to a matching page attribute', () => {
  const context = {
    window: { __acFindElement: () => null },
    document: {
      querySelector: () => {
        throw Error('No CSS fallback for missing analyzer references');
      },
    },
  };
  assert.equal(runInNewContext(INPUT_TYPE_JS('[data-ac-id="12"]'), context), null);
  assert.equal(runInNewContext(FIND_ELEMENT_JS('12'), context).error, 'Element not found');
});

test('a reference remains unavailable when the analyzer is absent', () => {
  const context = {
    window: {},
    document: {
      querySelector: () => {
        throw Error('No analyzer fallback');
      },
    },
  };
  assert.equal(runInNewContext(INPUT_TYPE_JS('[data-ac-id="12"]'), context), null);
});
