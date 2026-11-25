/**
 * Unit tests for lanes: the agent a browser command comes from rides along with
 * its work into every command, so an Oya desktop can give each agent its own tab.
 */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  inLane,
  inOwnLane,
  inCallerOrOwnLane,
  currentLane,
  laneFrom,
  laneMiddleware,
  LaneMap,
} from '../../../../../src/modules/browsers/connection/lane.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { describeCall } from '../../../../../src/modules/browsers/connection/reporter.ts';

it("stamps every command sent inside a lane's work with that lane, across awaits", async () => {
  const call = await inLane('agent-2', async () => {
    await new Promise((resolve) => setImmediate(resolve));
    return describeCall('b1', 'click', {}, 1000);
  });
  assert.equal(call.lane, 'agent-2');
  assert.equal(describeCall('b1', 'click', {}, 1000).lane, undefined, 'outside a lane, no lane');
});

it('keeps two agents apart when their work interleaves', async () => {
  const seen: (string | undefined)[] = [];
  const work = (lane: string) =>
    inLane(lane, async () => {
      await new Promise((resolve) => setImmediate(resolve));
      seen.push(currentLane());
    });
  await Promise.all([work('a'), work('b')]);
  assert.deepEqual(seen.sort(), ['a', 'b']);
});

it('runs a lane that is not a short name in no lane, so a caller cannot smuggle text into it', () => {
  for (const bad of ['', 'has space', 'x'.repeat(65), 42, { lane: 'a' }])
    assert.equal(
      inLane(bad, () => currentLane()),
      undefined,
    );
});

it("takes an external caller's lane from X-Oya-Agent, or ?agent= on its URL", () => {
  assert.equal(laneFrom({ headers: { 'x-oya-agent': 'research' } }), 'research');
  assert.equal(laneFrom({ headers: {}, query: { agent: 'mail' } }), 'mail');
});

it("keeps each agent's state on a browser apart, and gives a read outside a lane the latest", () => {
  const pages = new LaneMap<string>();
  inLane('tab-1', () => pages.set('b1', 'calendar'));
  inLane('tab-2', () => pages.set('b1', 'checkboxes'));
  assert.deepEqual(
    [inLane('tab-1', () => pages.get('b1')), inLane('tab-2', () => pages.get('b1')), pages.get('b1')],
    ['calendar', 'checkboxes', 'checkboxes'],
  );
  inLane('tab-2', () => pages.delete('b1'));
  assert.equal(
    inLane('tab-1', () => pages.get('b1')),
    'calendar',
    "one agent forgetting its page leaves the other's",
  );
  pages.delete('b1');
  assert.equal(
    inLane('tab-1', () => pages.get('b1')),
    undefined,
    'a browser that went takes every lane with it',
  );
});

it('keeps at most its cap of entries, the oldest going first', () => {
  const runs = new LaneMap<number>(2);
  for (const [lane, n] of [
    ['a', 1],
    ['b', 2],
    ['c', 3],
  ] as const)
    inLane(lane, () => runs.set('b1', n));
  assert.equal(
    inLane('a', () => runs.get('b1')),
    undefined,
  );
  assert.equal(
    inLane('c', () => runs.get('b1')),
    3,
  );
});

afterEach(() => mock.restoreAll());

/** Every message sent to a browser while the test runs, as [browserId, message]. */
function sends() {
  const sent: [string, any][] = [];
  mock.method(registry, 'send', (browserId: string, message: any) => (sent.push([browserId, message]), true));
  return sent;
}

it('runs work in a lane of its own, then tells the browser that lane has ended', async () => {
  const sent = sends();
  const lane = await inOwnLane('b1', async () => currentLane());
  assert.match(String(lane), /^[0-9a-f-]{36}$/);
  assert.deepEqual(sent, [['b1', { type: 'lane_end', lane }]]);
});

it('ends its lane when the work throws too', async () => {
  const sent = sends();
  await assert.rejects(
    inOwnLane('b1', async () => {
      throw new Error('portal down');
    }),
    /portal down/,
  );
  assert.equal(sent[0][1].type, 'lane_end');
});

it('ends no lane while the work is still waiting, as a run parked on a person is', async () => {
  const sent = sends();
  let resume = () => {};
  const run = inOwnLane('b1', () => new Promise<void>((resolve) => (resume = resolve)));
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(sent, []);
  resume();
  await run;
  assert.equal(sent.length, 1);
});

it('sends the end before the work reports done, so the next run never finds the lane still held', async () => {
  const sent = sends();
  const seenAtDone = await inOwnLane('b1', async () => 'ok').then(() => sent.length);
  assert.equal(seenAtDone, 1);
});

it("keeps a caller's lane for the work, and ends no lane the caller named", async () => {
  const sent = sends();
  const lane = await inLane('sdk-script', () => inCallerOrOwnLane('b1', async () => currentLane()));
  assert.equal(lane, 'sdk-script');
  assert.deepEqual(sent, []);
});

it('gives work a lane of its own, ended after, when no caller named one', async () => {
  const sent = sends();
  const lane = await inCallerOrOwnLane('b1', async () => currentLane());
  assert.match(String(lane), /^[0-9a-f-]{36}$/);
  assert.deepEqual(sent, [['b1', { type: 'lane_end', lane }]]);
});

it('runs the rest of a request in the lane its X-Oya-Agent header names', async () => {
  const seen = await new Promise((resolve) =>
    laneMiddleware({ headers: { 'x-oya-agent': 'sdk-1' } }, {}, async () => {
      await new Promise((r) => setImmediate(r));
      resolve(currentLane());
    }),
  );
  assert.equal(seen, 'sdk-1');
});

it('runs a request that names no lane in none', () => {
  let seen: string | undefined = 'unset';
  laneMiddleware({ headers: {} }, {}, () => (seen = currentLane()));
  assert.equal(seen, undefined);
});
