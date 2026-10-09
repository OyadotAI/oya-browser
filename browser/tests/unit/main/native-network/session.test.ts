/** Private native pipelines observe only their exact enabled pages and cancel held work on teardown. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { NativeNetworkSession } from '../../../../src/main/native-network/session.ts';
/** Native session seams capture callbacks rather than simulating any protocol transport. */
function fixture() {
  const hooks: Record<string, any> = {},
    events: any[] = [];
  const request = Object.fromEntries(
    [
      'onBeforeRequest',
      'onResponseStarted',
      'onBeforeRedirect',
      'onCompleted',
      'onErrorOccurred',
      '_setOyaBodyListener',
    ].map((k) => [
      k,
      (f: any) => {
        hooks[k] = f;
      },
    ]),
  );
  const session = { webRequest: request } as any;
  const contents = Object.assign(new EventEmitter(), {
    session,
    isDestroyed: () => false,
    _supportsOyaFrameLifecycle: () => true,
  }) as any;
  let allowed = true;
  const network = new NativeNetworkSession(session, {
    allowed: () => allowed,
    allowedURL: () => true,
    target: () => 'page',
    frameId: (_contents, frame) => (frame === contents.mainFrame ? 'page' : 'child'),
  });
  const details = {
    id: 1,
    frame: { parent: null, frameToken: 'main', processId: 1, routingId: 2, framesInSubtree: [] as any[] },
    webContents: contents,
    url: 'https://example.test',
    method: 'GET',
    resourceType: 'xhr',
  };
  contents.mainFrame = details.frame;
  details.frame.framesInSubtree = [details.frame];
  return {
    network,
    hooks,
    contents,
    details,
    events,
    emit: (m: string, p: any) => events.push({ m, p }),
    human: () => {
      allowed = false;
    },
  };
}
test('human activity never creates exposed network records or retained response bodies', async () => {
  const f = fixture();
  f.network.watch(f.contents, 'OyaNetwork', f.emit);
  f.human();
  const decisions: any[] = [];
  f.hooks.onBeforeRequest(f.details, (d: any) => decisions.push(d));
  assert.deepEqual(decisions, [{}]);
  assert.deepEqual(f.events, []);
  f.network.dispose();
  assert.equal(f.hooks.onCompleted, null);
  assert.equal(f.hooks._setOyaBodyListener, null);
});
test('destroyed pages cancel paused callbacks and revoke retained response readers', async () => {
  const f = fixture();
  f.network.watch(f.contents, 'OyaNetwork', f.emit);
  f.network.watch(f.contents, 'Fetch', f.emit);
  const decisions: any[] = [];
  f.hooks.onBeforeRequest(f.details, (d: any) => decisions.push(d));
  const pending = assert.rejects(f.network.body(f.contents, f.events[0].p.requestId), /revoked/);
  f.contents.emit('destroyed');
  await pending;
  assert.deepEqual(decisions, [{ cancel: true }]);
  assert.equal(f.contents.listenerCount('destroyed'), 0);
  f.network.dispose();
});
test('a disposed native frame getter fails closed and consumes its callback once', () => {
  const f = fixture();
  f.network.watch(f.contents, 'Fetch', f.emit);
  const d = {
      ...f.details,
      get frame() {
        throw Error('disposed frame');
      },
    },
    decisions: any[] = [];
  f.hooks.onBeforeRequest(d, (decision: any) => decisions.push(decision));
  assert.deepEqual(decisions, [{ cancel: true }]);
  f.network.dispose();
});

test('child requests report the shared native frame identity rather than the main page', () => {
  const f = fixture();
  const child = { parent: f.details.frame, frameToken: 'child', processId: 2, routingId: 3 };
  f.details.frame.framesInSubtree.push(child);
  f.network.watch(f.contents, 'OyaNetwork', f.emit);
  f.network.watch(f.contents, 'Fetch', f.emit);
  const decisions: any[] = [];
  f.hooks.onBeforeRequest({ ...f.details, frame: child }, (d: any) => decisions.push(d));
  assert.equal(f.events[0].p.frameId, 'child');
  assert.equal(f.events[1].p.frameId, 'child');
  assert.deepEqual(decisions, []);
  f.contents.emit('did-frame-navigate', {}, 'https://new.test', 200, 'OK', false, 2, 3);
  assert.deepEqual(decisions, [{ cancel: true }]);
  f.network.dispose();
  assert.equal(f.contents.listenerCount('oya-frame-tree-changed'), 0);
  assert.equal(f.contents.listenerCount('did-frame-navigate'), 0);
});
test('missing and foreign native frame attribution never invents a parent page', () => {
  const f = fixture();
  f.network.watch(f.contents, 'OyaNetwork', f.emit);
  for (const frame of [null, { detached: false, parent: null }]) {
    const decisions: any[] = [];
    f.hooks.onBeforeRequest({ ...f.details, frame }, (d: any) => decisions.push(d));
    assert.deepEqual(decisions, [{}]);
  }
  assert.deepEqual(f.events, []);
  f.network.dispose();
});

test('old engines fail before enabling response capture or retaining page listeners', () => {
  const f = fixture();
  delete f.contents._supportsOyaFrameLifecycle;
  assert.throws(() => f.network.watch(f.contents, 'OyaNetwork', f.emit), /lifecycle/);
  assert.equal(f.hooks._setOyaBodyListener, undefined);
  assert.equal(f.contents.listenerCount('destroyed'), 0);
  f.network.dispose();
});

test('filter updates preserve held callbacks and nonmatching requests continue normally', () => {
  const f = fixture(),
    decisions: any[] = [];
  const stop = f.network.watch(f.contents, 'Fetch', f.emit, { patterns: [{ urlPattern: '*/selected' }] });
  f.hooks.onBeforeRequest(f.details, (d: any) => decisions.push(d));
  assert.deepEqual(decisions, [{}]);
  assert.equal(f.events.length, 0);
  f.hooks.onBeforeRequest({ ...f.details, id: 2, url: 'https://example.test/selected' }, (d: any) => decisions.push(d));
  const held = f.events[0].p.requestId;
  stop.update!({ patterns: [] });
  assert.deepEqual(decisions, [{}]);
  f.network.resolve(f.contents, held, false);
  assert.deepEqual(decisions, [{}, { cancel: false }]);
  stop();
  f.network.dispose();
});
test('stale cleanup and updates cannot affect a replacement interception owner', () => {
  const f = fixture();
  const old = f.network.watch(f.contents, 'Fetch', f.emit);
  old();
  const current = f.network.watch(f.contents, 'Fetch', f.emit);
  old();
  assert.throws(() => old.update!({ patterns: [] }), /revoked/);
  const decisions: any[] = [];
  f.hooks.onBeforeRequest(f.details, (d: any) => decisions.push(d));
  assert.deepEqual(decisions, []);
  f.human();
  assert.throws(() => current.update!({ patterns: [] }), /revoked/);
  current();
  assert.deepEqual(decisions, [{ cancel: true }]);
  f.network.dispose();
});
test('failure reasons require engine support and rejected requests preserve the original continuation', () => {
  const f = fixture(),
    decisions: any[] = [];
  f.network.watch(f.contents, 'Fetch', f.emit);
  f.hooks.onBeforeRequest(f.details, (d: any) => decisions.push(d));
  const id = f.events.find((e) => e.m === 'Fetch.requestPaused').p.requestId;
  assert.throws(() => f.network.resolve(f.contents, id, true, 'TimedOut'), /engine/);
  assert.throws(() => f.network.resolve(f.contents, id, true, 'invented'), /reason/);
  assert.deepEqual(decisions, []);
  f.contents.session.webRequest._supportsOyaRequestErrors = () => true;
  assert.throws(() => f.network.resolve(f.contents, id, false, 'TimedOut'), /continued/);
  f.network.resolve(f.contents, id, true, 'TimedOut');
  assert.deepEqual(decisions, [{ cancel: true, _oyaErrorReason: 'TimedOut' }]);
  assert.throws(() => f.network.resolve(f.contents, id, true, 'TimedOut'), /foreign/);
  f.network.dispose();
});
test('human revocation cannot select a replacement network failure result', () => {
  const f = fixture(),
    decisions: any[] = [];
  f.contents.session.webRequest._supportsOyaRequestErrors = () => true;
  f.network.watch(f.contents, 'Fetch', f.emit);
  f.hooks.onBeforeRequest(f.details, (d: any) => decisions.push(d));
  const id = f.events.find((e) => e.m === 'Fetch.requestPaused').p.requestId;
  f.human();
  assert.throws(() => f.network.resolve(f.contents, id, true, 'Aborted'), /authorized/);
  assert.deepEqual(decisions, [{ cancel: true }]);
  f.network.dispose();
});
