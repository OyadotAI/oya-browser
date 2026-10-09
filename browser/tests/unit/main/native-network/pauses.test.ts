/** Native continuations are single-use, deadline-bound and protected by current owner/egress policy. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativePauses } from '../../../../src/main/native-network/pauses.ts';
/** Browser policy can change while the renderer's original request is paused. */
function fixture() {
  let allowed = true,
    egress = true;
  const contents = {} as any,
    calls: any[] = [],
    events: any[] = [];
  const frame = { detached: false, frameToken: 'first', processId: 1, routingId: 2 } as any;
  contents.isDestroyed = () => false;
  contents.mainFrame = { framesInSubtree: [frame] };
  const pauses = new NativePauses({
    allowed: () => allowed,
    allowedURL: () => egress,
    target: () => 'page',
    frameId: () => 'frame',
  });
  const details = {
    frame,
    id: 1,
    url: 'https://example.test',
    method: 'GET',
    resourceType: 'xhr',
    webContents: contents,
  };
  pauses.hold(
    details,
    (d) => calls.push(d),
    (_m, p) => events.push(p),
    'network',
  );
  return {
    pauses,
    contents,
    frame,
    calls,
    id: events[0].requestId,
    human: () => {
      allowed = false;
    },
    deny: () => {
      egress = false;
    },
  };
}
test('native continuations reject foreign pages and consume the callback exactly once', () => {
  const f = fixture();
  assert.throws(() => f.pauses.resolve({} as any, f.id, false), /foreign/);
  assert.deepEqual(f.calls, []);
  f.pauses.resolve(f.contents, f.id, false);
  assert.deepEqual(f.calls, [{ cancel: false }]);
  assert.throws(() => f.pauses.resolve(f.contents, f.id, false), /foreign/);
  f.pauses.clear();
  assert.equal(f.calls.length, 1);
});
test('human takeover and changed egress policy cancel held requests instead of continuing', () => {
  for (const revoke of ['human', 'deny'] as const) {
    const f = fixture();
    f[revoke]();
    assert.throws(() => f.pauses.resolve(f.contents, f.id, false), /authorized/);
    assert.deepEqual(f.calls, [{ cancel: true }]);
    f.pauses.clear();
    assert.equal(f.calls.length, 1);
  }
});
test('a silent or disconnected agent cannot strand a paused request indefinitely', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  t.mock.timers.tick(10000);
  assert.deepEqual(f.calls, [{ cancel: true }]);
  f.pauses.clear();
  assert.equal(f.calls.length, 1);
});

test('renderer replacement cannot reuse a paused request even when the wrapper survives', () => {
  const f = fixture();
  f.frame.frameToken = 'replacement';
  assert.throws(() => f.pauses.resolve(f.contents, f.id, false), /authorized/);
  assert.deepEqual(f.calls, [{ cancel: true }]);
});
test('same-renderer document commit cancels the exact frame and not a sibling route', () => {
  const f = fixture();
  f.pauses.changed(f.contents, 1, 3);
  assert.deepEqual(f.calls, []);
  f.pauses.changed(f.contents, 1, 2);
  assert.deepEqual(f.calls, [{ cancel: true }]);
  assert.throws(() => f.pauses.resolve(f.contents, f.id, false), /foreign/);
});
test('native graph removal revokes held requests without waiting for the agent or deadline', () => {
  const f = fixture();
  f.contents.mainFrame.framesInSubtree = [];
  f.pauses.changed(f.contents);
  assert.deepEqual(f.calls, [{ cancel: true }]);
});
test('native request failure consumes its continuation exactly once', () => {
  const f = fixture();
  f.pauses.cancelRequest(1);
  f.pauses.cancelRequest(1);
  assert.deepEqual(f.calls, [{ cancel: true }]);
});
