/** Model writes remain with their original owner and destination across login, catalog waits and cancellation. */
import { afterEach, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { setImmediate as turn } from 'node:timers/promises';
import { ModelSettings } from '../../../../src/main/connection/model-settings.ts';
import { MODEL_SETTINGS_TIMEOUT_MS } from '../../../../src/main/connection/constants.ts';

/** A native shell lifetime with no browser or file access. */
function owner(id = 1): any {
  const shell = new EventEmitter() as any;
  shell.id = id;
  shell.isDestroyed = () => false;
  return shell;
}
/** Every API request is replaced; the workspace is mutable to reproduce races. */
function fixture() {
  const deps: any = {
    config: { values: { serverUrl: 'wss://oya.test/ws', apiKey: 'workspace-a' } },
    socket: { ready: true, browserId: 'desktop' },
  };
  return { deps, service: new ModelSettings(deps), owner: owner() };
}
const config = {
  llm_provider: 'gemini',
  llm_catalog: [{ id: 'gemini', model: 'catalog-default' }],
  effective: { hasLlmKey: true },
};
const choice = { provider: 'gemini', key: 'synthetic-provider-key', model: 'catalog-default', baseUrl: '' };
afterEach(() => {
  mock.restoreAll();
  mock.timers.reset();
});
/** Native AbortSignal timers are replaced at the seam so fake time can drive the actual fetch signal. */
function fakeDeadlines(): void {
  mock.timers.enable({ apis: ['setTimeout'] });
  mock.method(AbortSignal, 'timeout', (ms) => {
    const abort = new AbortController();
    setTimeout(() => abort.abort(new DOMException('Catalog deadline', 'TimeoutError')), ms);
    return abort.signal;
  });
}
/** Network work remains pending until the supplied request signal really aborts it. */
function stalledFetch(_url: unknown, init: any): Promise<never> {
  return new Promise((_resolve, reject) => {
    init.signal.addEventListener('abort', () => reject(init.signal.reason), { once: true });
  });
}
it('a catalog deadline aborts the underlying fetch and allows a fresh retry', async () => {
  fakeDeadlines();
  const f = fixture(),
    fetch = mock.method(globalThis, 'fetch', stalledFetch);
  f.service.beginFlow(f.owner);
  const failed = assert.rejects(f.service.status(f.owner), { name: 'TimeoutError' });
  mock.timers.tick(MODEL_SETTINGS_TIMEOUT_MS);
  await failed;
  fetch.mock.mockImplementation(async () => ({ ok: true, json: async () => config }) as any);
  assert.deepEqual((await f.service.status(f.owner)).catalog, config.llm_catalog);
  f.service.cancel();
});
it('leaving Technical setup aborts a pending catalog and removes owner subscriptions', async () => {
  const f = fixture();
  mock.method(globalThis, 'fetch', stalledFetch);
  f.service.beginFlow(f.owner);
  const failed = assert.rejects(f.service.status(f.owner), { name: 'AbortError' });
  f.service.cancel(f.owner.id);
  await failed;
  assert.equal(f.owner.listenerCount('destroyed'), 0);
});
it('a catalog reply from a previous workspace cannot populate the Technical form', async () => {
  const f = fixture(),
    answer = Promise.withResolvers<any>();
  mock.method(globalThis, 'fetch', () => answer.promise);
  f.service.beginFlow(f.owner);
  const failed = assert.rejects(f.service.status(f.owner), /workspace changed/);
  f.deps.config.values.apiKey = 'another-workspace';
  answer.resolve({ ok: true, json: async () => config });
  await failed;
  f.service.cancel();
});
it('Technical setup refuses a missing real catalog before sending any provider key', async () => {
  const f = fixture();
  const fetch = mock.method(
    globalThis,
    'fetch',
    async () => ({ ok: true, json: async () => ({ effective: {} }) }) as any,
  );
  f.service.beginFlow(f.owner);
  await assert.rejects(f.service.write(f.owner, choice), /server needs an update/);
  assert.equal(fetch.mock.callCount(), 1);
  assert.equal((fetch.mock.calls[0].arguments[1] as any).method, 'GET');
  f.service.cancel();
});
it('a provider draft cannot save to a workspace changed after Connect was clicked', async () => {
  const f = fixture(),
    fetch = mock.method(globalThis, 'fetch', async () => {
      throw Error('must not call');
    });
  f.service.beginFlow(f.owner);
  f.deps.config.values.serverUrl = 'wss://another.test/ws';
  await assert.rejects(f.service.write(f.owner, choice), /workspace changed/);
  assert.equal(fetch.mock.callCount(), 0);
  f.service.cancel();
});
it('only its same-owner correlated login can adopt the new workspace key', async () => {
  const f = fixture();
  f.deps.config.values.apiKey = '';
  f.service.beginFlow(f.owner);
  f.service.adoptLogin(2, 'wrong-owner-key');
  f.deps.config.values.apiKey = 'wrong-owner-key';
  await assert.rejects(f.service.write(f.owner, choice), /workspace changed/);
  f.service.cancel();
  f.deps.config.values.apiKey = '';
  f.service.beginFlow(f.owner);
  f.service.adoptLogin(1, 'authenticated-key');
  f.deps.config.values.apiKey = 'authenticated-key';
  const fetch = mock.method(
    globalThis,
    'fetch',
    async (_url, init: any) => ({ ok: true, json: async () => (init.method === 'GET' ? config : { ok: true }) }) as any,
  );
  await f.service.write(f.owner, choice);
  assert.equal(fetch.mock.calls.length, 2);
  assert.equal((fetch.mock.calls[1].arguments[1] as any).headers.Authorization, 'Bearer authenticated-key');
  f.service.cancel();
});
it('changing workspace during a catalog read cannot send the typed provider key elsewhere', async () => {
  const f = fixture(),
    answer = Promise.withResolvers<any>();
  const fetch = mock.method(globalThis, 'fetch', () => answer.promise);
  const pending = f.service.write(f.owner, choice);
  await turn();
  f.deps.config.values.apiKey = 'workspace-b';
  answer.resolve({ ok: true, json: async () => config });
  await assert.rejects(pending, /workspace changed/);
  assert.equal(fetch.mock.callCount(), 1);
});
it('owner closure aborts the underlying pending write and removes its subscriptions', async () => {
  const f = fixture();
  let signal: AbortSignal | undefined;
  mock.method(globalThis, 'fetch', async (_url, init: any) => {
    if (init.method === 'GET') return { ok: true, json: async () => config } as any;
    signal = init.signal;
    return new Promise((_resolve, reject) =>
      signal!.addEventListener('abort', () => reject(Error('aborted')), { once: true }),
    );
  });
  const pending = f.service.write(f.owner, choice),
    failed = assert.rejects(pending, /aborted/);
  await turn();
  f.owner.emit('destroyed');
  await failed;
  assert(signal?.aborted);
  assert.equal(f.owner.listenerCount('destroyed'), 0);
});
