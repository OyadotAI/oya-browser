/**
 * Unit tests for the control routes' guards: the caller's key, the
 * administrator guard, and the check that a browser is the caller's own.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { admin, key, requireOwnBrowser } from '../../../../../src/modules/control/http/guards.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';

/** A response that records its status and JSON. */
const response = () => {
  const res: any = {};
  res.status = mock.fn((code) => ((res.code = code), res));
  res.json = mock.fn((body) => ((res.body = body), res));
  return res;
};

describe('key', () => {
  it('is the project key the caller authenticated as', () => {
    assert.equal(key({ principal: { key: 'key-a' } }), 'key-a');
  });
});

describe('admin', () => {
  it('lets an administrator through', () => {
    const next = mock.fn();
    admin({ principal: { role: 'administrator' } }, response(), next);
    assert.equal(next.mock.callCount(), 1);
  });

  it('answers 403 to anyone else', () => {
    const next = mock.fn(),
      res = response();
    admin({ principal: { role: 'operator' } }, res, next);
    assert.equal(next.mock.callCount(), 0);
    assert.deepEqual([res.code, res.body], [403, { error: 'Administrator permission required' }]);
  });
});

describe('requireOwnBrowser', () => {
  it('passes for a browser connected here under the caller’s key', () => {
    connectBrowser('b-guard', 'key-a');
    try {
      assert.doesNotThrow(() => requireOwnBrowser({ params: { id: 'b-guard' }, principal: { key: 'key-a' } }));
    } finally {
      disconnectBrowser('b-guard');
    }
  });

  it('answers 404 for another key’s browser or one not connected here', () => {
    connectBrowser('b-guard', 'key-b');
    try {
      for (const id of ['b-guard', 'missing'])
        assert.throws(() => requireOwnBrowser({ params: { id }, principal: { key: 'key-a' } }), {
          status: 404,
          message: 'Browser not connected',
        });
    } finally {
      disconnectBrowser('b-guard');
    }
  });
});
