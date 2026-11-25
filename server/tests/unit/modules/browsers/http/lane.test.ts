/**
 * Unit tests for ending a caller's lane: an SDK browser on a person's desktop calls it when it is
 * done, so the desktop closes the tab the lane worked in.
 */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { endCallerLane } from '../../../../../src/modules/browsers/http/lane.ts';
import { inLane } from '../../../../../src/modules/browsers/connection/lane.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';

afterEach(() => mock.restoreAll());

/** A response that records its status and body. */
function response() {
  const res = { statusCode: 200, body: undefined as unknown };
  return Object.assign(res, {
    status: (code: number) => ((res.statusCode = code), res),
    json: (body: unknown) => ((res.body = body), res),
  });
}

it("tells the browser the caller's lane has ended", () => {
  const sent: [string, unknown][] = [];
  mock.method(registry, 'send', (id: string, msg: unknown) => (sent.push([id, msg]), true));
  const res = response();
  inLane('sdk-script', () => endCallerLane({ params: { browserId: 'b1' } }, res));
  assert.deepEqual(sent, [['b1', { type: 'lane_end', lane: 'sdk-script' }]]);
  assert.deepEqual(res.body, { ok: true, lane: 'sdk-script' });
});

it('refuses a caller that named no lane, and ends nothing', () => {
  const send = mock.method(registry, 'send', () => true);
  const res = response();
  endCallerLane({ params: { browserId: 'b1' } }, res);
  assert.equal(res.statusCode, 400);
  assert.equal(send.mock.callCount(), 0);
});

it("refuses to end a lane that is not an SDK browser's, such as an MCP agent's default lane", () => {
  const send = mock.method(registry, 'send', () => true);
  const res = response();
  inLane('default', () => endCallerLane({ params: { browserId: 'b1' } }, res));
  assert.equal(res.statusCode, 400);
  assert.equal(send.mock.callCount(), 0);
});
