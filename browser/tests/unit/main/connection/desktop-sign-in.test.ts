/** Correlated login keeps the captured server, owner and cancellation boundary without real auth or network. */
import { afterEach, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { setImmediate as turn } from 'node:timers/promises';
import { DesktopSignIn } from '../../../../src/main/connection/desktop-sign-in.ts';
import { DESKTOP_LOGIN_TTL_MS } from '../../../../src/main/connection/constants.ts';

/** Shell identity has a native lifetime but no Electron dependency. */
function owner(id = 1) {
  const value: any = new EventEmitter();
  value.id = id;
  value.closed = false;
  value.isDestroyed = () => value.closed;
  return value;
}
/** All credentials are synthetic and every callback and request is held in memory. */
function fixture() {
  const opened: string[] = [],
    applied: unknown[] = [],
    listeners: any[] = [];
  const config = { values: { serverUrl: 'wss://oya.test/ws', apiKey: '' } };
  const deps: any = {
    config,
    electron: {
      shell: {
        openExternal: async (url) => {
          opened.push(url);
        },
      },
    },
    deepLinks: {
      retarget: (target) => {
        applied.push(target);
        Object.assign(config.values, target);
      },
    },
    socket: {
      connects: 0,
      connect() {
        this.connects++;
      },
    },
  };
  const callback: any = async (state, signal) => {
    const done = Promise.withResolvers<string>();
    const value = {
      state,
      signal,
      url: 'http://127.0.0.1:41777/oya-desktop/callback',
      done: done.promise,
      resolve: done.resolve,
      closed: false,
      close() {
        this.closed = true;
        done.reject(Error('cancelled'));
      },
    };
    void done.promise.catch(() => {});
    signal.addEventListener('abort', () => done.reject(Error('cancelled')), { once: true });
    listeners.push(value);
    return value;
  };
  const service = new DesktopSignIn(deps, callback);
  return { service, deps, config, opened, applied, listeners };
}
afterEach(() => mock.restoreAll());
it('redeems only at the captured server and connects without a second dialog', async () => {
  const f = fixture();
  const fetch = mock.method(
    globalThis,
    'fetch',
    async () => ({ ok: true, json: async () => ({ apiKey: 'synthetic-workspace-key', persona: 'work' }) }) as any,
  );
  const pending = f.service.start(owner());
  await turn();
  const url = new URL(f.opened[0]);
  assert.equal(url.origin, 'https://oya.test');
  assert.match(url.searchParams.get('state')!, /^[a-f0-9]{64}$/);
  assert.equal(url.searchParams.get('callback'), f.listeners[0].url);
  f.listeners[0].resolve('one-use-code');
  await pending;
  const [target, init] = fetch.mock.calls[0].arguments as any;
  assert.equal(target, 'https://oya.test/api/pairing/claim');
  assert.equal(init.redirect, 'error');
  assert.deepEqual(JSON.parse(init.body), { code: 'one-use-code' });
  assert.equal(f.deps.socket.connects, 1);
  assert.deepEqual(f.applied, [{ serverUrl: 'wss://oya.test/ws', apiKey: 'synthetic-workspace-key', persona: 'work' }]);
  assert(f.listeners[0].closed);
});
it('a changed workspace refuses a late callback before redeeming it', async () => {
  const f = fixture(),
    fetch = mock.method(globalThis, 'fetch', async () => {
      throw Error('must not call');
    });
  const pending = f.service.start(owner());
  await turn();
  f.config.values.apiKey = 'another-project';
  f.listeners[0].resolve('late-code');
  await assert.rejects(pending, /workspace changed/);
  assert.equal(fetch.mock.callCount(), 0);
  assert.equal(f.applied.length, 0);
});
it('owner closure cancels only that window and cannot pair either late return', async () => {
  const f = fixture(),
    first = owner(1),
    second = owner(2);
  const one = f.service.start(first),
    two = f.service.start(second);
  const rejectedOne = assert.rejects(one),
    rejectedTwo = assert.rejects(two);
  await turn();
  first.closed = true;
  first.emit('destroyed');
  await rejectedOne;
  assert(f.listeners[0].signal.aborted);
  assert(!f.listeners[1].signal.aborted);
  f.service.cancel(2);
  await rejectedTwo;
  assert.equal(f.applied.length, 0);
});
it('expiry also settles a stalled native opener and removes owner subscriptions', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture(),
    shell = owner();
  f.deps.electron.shell.openExternal = () => new Promise(() => {});
  const pending = f.service.start(shell),
    rejected = assert.rejects(pending, /didn’t finish|cancelled/);
  await turn();
  t.mock.timers.tick(DESKTOP_LOGIN_TTL_MS);
  await rejected;
  assert(f.listeners[0].closed);
  assert.equal(shell.listenerCount('destroyed'), 0);
});
