/** Native storage lifecycle regressions using only an immutable session and a fallible transport. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { NativeStorageSync } from '../../../../src/main/sync/native-storage.ts';
import { STORAGE_CAPTURE_TIMEOUT_MS } from '../../../../src/main/sync/constants.ts';
const origin = 'https://example.test';
/** A controllable native reply, without sleeps or network access. */
function deferred() {
  let resolve!: (value?: any) => void;
  const promise = new Promise<any>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
/** Native-only session seam; no page evaluation or debugging endpoint exists. */
function fixture() {
  const session: any = new EventEmitter();
  const state = { entries: [] as string[][], online: true, accept: true, throws: false, reads: 0 };
  const sent: any[] = [],
    restored: any[] = [],
    unwatched: string[] = [];
  session._readOyaLocalStorage = async () => {
    state.reads++;
    return state.entries;
  };
  session._restoreOyaLocalStorage = async (...args: any[]) => {
    restored.push(args);
    state.entries = args[1];
    return state.entries;
  };
  session._watchOyaLocalStorage = async () => {};
  session._unwatchOyaLocalStorage = (key: string) => unwatched.push(key);
  const sync = new NativeStorageSync({
    session,
    ready: () => state.online,
    send: (message) => {
      if (state.throws) throw Error('socket closed');
      if (state.accept) sent.push(message);
      return state.accept;
    },
  });
  return { session, state, sent, restored, unwatched, sync };
}
it('refuses a missing native capability without a fallback', () => {
  assert.throws(() => new NativeStorageSync({ session: {} } as any), /capability unavailable/);
});
it('validates the entire import before any mutation', () => {
  const f = fixture();
  assert.throws(() => f.sync.initialize({ [origin]: { account: 'Alice' }, 'file:///secret': {} }), /origin/);
  assert.equal(f.restored.length, 0);
  f.sync.dispose();
});
it('flush waits for exclusive restoration and observer readiness', async () => {
  const f = fixture(),
    restore = deferred(),
    watch = deferred();
  f.session._restoreOyaLocalStorage = () => restore.promise;
  f.session._watchOyaLocalStorage = () => watch.promise;
  const initialized = f.sync.initialize({ [origin]: {} });
  const flushed = f.sync.flush();
  assert.throws(() => f.sync.initialize({}), /already started/);
  assert.equal(f.sent.length, 0);
  restore.resolve([]);
  watch.resolve();
  await initialized;
  assert.equal(await flushed, true);
  assert.deepEqual(f.sent[0].origins, { [origin]: {} });
  f.sync.dispose();
});
it('offline, refused, and throwing transports retain the newest logout state', async () => {
  const f = fixture();
  await f.sync.initialize({ [origin]: { account: 'Alice' } });
  f.state.online = false;
  assert.equal(await f.sync.flush(), false);
  f.state.entries = [];
  f.session.emit('oya-local-storage-changed', {}, origin, true);
  f.state.online = true;
  f.state.accept = false;
  assert.equal(await f.sync.flush(), false);
  f.state.throws = true;
  assert.equal(await f.sync.flush(), false);
  f.state.throws = false;
  f.state.accept = true;
  assert.equal(await f.sync.flush(), true);
  assert.deepEqual(f.sent, [{ type: 'storage_changed', origins: { [origin]: {} } }]);
  assert.equal(f.restored.length, 1, 'logout must never trigger restoration');
  f.sync.dispose();
});
it('mutations during a read coalesce and replace the stale snapshot', async () => {
  const f = fixture(),
    read = deferred();
  await f.sync.watch(origin);
  let reads = 0;
  f.session._readOyaLocalStorage = () => (++reads === 1 ? read.promise : Promise.resolve([['account', 'Bob']]));
  f.session.emit('oya-local-storage-changed', {}, origin, true);
  f.session.emit('oya-local-storage-changed', {}, origin, true);
  const flushed = f.sync.flush();
  read.resolve([['account', 'Alice']]);
  await flushed;
  assert.equal(f.sent[0].origins[origin].account, 'Bob');
  assert.equal(reads, 2);
  f.sync.dispose();
});
it('disposal fences an in-flight snapshot and detaches native observation', async () => {
  const f = fixture(),
    read = deferred();
  await f.sync.watch(origin);
  const started = deferred();
  f.session._readOyaLocalStorage = () => {
    started.resolve();
    return read.promise;
  };
  f.session.emit('oya-local-storage-changed', {}, origin, true);
  await started.promise;
  f.sync.dispose();
  f.sync.dispose();
  read.resolve([['secret', 'old-persona']]);
  await assert.rejects(f.sync.flush(), /disposed/);
  assert.equal(f.sent.length, 0);
  assert.equal(f.session.listenerCount('oya-local-storage-changed'), 0);
  assert.deepEqual(f.unwatched, [origin]);
});
it('a discarded restore cannot install a watcher or restore subsequent origins', async () => {
  const f = fixture(),
    restore = deferred();
  f.session._restoreOyaLocalStorage = () => restore.promise;
  const pending = f.sync.initialize({ [origin]: {}, 'https://second.test': {} });
  await Promise.resolve();
  f.sync.dispose();
  restore.resolve([]);
  await assert.rejects(pending, /disposed/);
  assert.equal(f.state.reads, 0);
  assert.equal(f.sent.length, 0);
});
it('ignores foreign origins and isolates identical origins in separate sessions', async () => {
  const a = fixture(),
    b = fixture();
  await a.sync.watch(origin);
  await b.sync.watch(origin);
  a.session.emit('oya-local-storage-changed', {}, 'https://foreign.test', false);
  a.session.emit('oya-local-storage-changed', {}, origin, false);
  await assert.rejects(a.sync.flush(), /disconnected/);
  assert.equal(await b.sync.flush(), true);
  a.sync.dispose();
  b.sync.dispose();
});
it('sanitizes native errors and never reports a failed snapshot as saved', async () => {
  const f = fixture();
  f.session._readOyaLocalStorage = async () => {
    throw Error('credential-secret');
  };
  await assert.rejects(f.sync.watch(origin), /^Error: Native storage observation failed$/);
  await assert.rejects(f.sync.flush(), /observation failed/);
  assert.equal(f.sent.length, 0);
  f.sync.dispose();
});
it('times out an unresponsive native observer without publishing state', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  f.session._watchOyaLocalStorage = () => new Promise(() => {});
  const pending = f.sync.watch(origin);
  const rejected = assert.rejects(pending, /observation failed/);
  t.mock.timers.tick(STORAGE_CAPTURE_TIMEOUT_MS);
  await rejected;
  assert.equal(f.sent.length, 0);
  f.sync.dispose();
});

it('sanitizes synchronous native failures as well as promise rejections', async () => {
  const f = fixture();
  f.session._restoreOyaLocalStorage = () => {
    throw Error('credential-secret');
  };
  await assert.rejects(f.sync.initialize({ [origin]: {} }), /^Error: Native storage operation failed$/);
  await assert.rejects(f.sync.flush(), /operation failed/);
  assert.deepEqual(f.sent, []);
  f.sync.dispose();
});

it('coalesces duplicate subscriptions and rejects readiness after disposal', async () => {
  const f = fixture(),
    ready = deferred();
  let calls = 0;
  f.session._watchOyaLocalStorage = () => {
    calls++;
    return ready.promise;
  };
  const pending = f.sync.watch(origin);
  assert.equal(f.sync.watch(origin), pending);
  await Promise.resolve();
  f.sync.dispose();
  ready.resolve();
  await assert.rejects(pending, /observation failed/);
  assert.equal(calls, 1);
  assert.equal(f.state.reads, 0);
  assert.deepEqual(f.unwatched, [origin]);
});
