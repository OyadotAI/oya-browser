/**
 * Unit tests for the pieces the browser and pool routes share: refusing a
 * missing or server-internal action, lifting socket timeouts, and auditing an
 * action on a browser.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { auditBrowser, noTimeouts, refuseAction } from '../../../../../src/modules/browsers/http/helpers.ts';
import { fingerprint, recent } from '../../../../../src/platform/audit.ts';
import { FakeResponse, fakeRequest } from '../../../support/browsers.ts';

describe('route helpers', () => {
  it('answers 400 for a missing action', () => {
    const res = new FakeResponse();
    assert.ok(refuseAction(res, undefined));
    assert.deepEqual([res.statusCode, res.body], [400, { error: 'Missing action' }]);
  });

  it('answers 403 for an action only the server may send', () => {
    for (const action of ['evaluate_raw', 'evaluate', 'record']) {
      const res = new FakeResponse();
      assert.ok(refuseAction(res, action));
      assert.deepEqual([res.statusCode, res.body.error], [403, `${action} is not available through this API`]);
    }
  });

  it('lets an ordinary action through without answering', () => {
    const res = new FakeResponse();
    assert.equal(refuseAction(res, 'click'), null);
    assert.equal(res.body, undefined);
  });

  it('turns off the socket timeout on both sides', () => {
    const req = fakeRequest();
    const res = new FakeResponse();
    req.setTimeout = mock.fn();
    res.setTimeout = mock.fn();
    noTimeouts(req, res);
    assert.deepEqual(req.setTimeout.mock.calls[0].arguments, [0]);
    assert.deepEqual((res.setTimeout as any).mock.calls[0].arguments, [0]);
  });

  it('audits an action on a browser as the caller', () => {
    auditBrowser(fakeRequest({ key: 'k-help' }), 'helpers.test', 'b-1', { n: 1 }, 'denied');
    const [row] = recent({ action: 'helpers.test' });
    assert.deepEqual(
      [row.actor, row.target_type, row.target_id, row.outcome],
      [fingerprint('k-help'), 'browser', 'b-1', 'denied'],
    );
  });
});
