/** The incognito windows' shared session: protected like an agent context, shared while open, wiped after. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { IncognitoSession, isIncognito } from '../../../../src/main/incognito/index.ts';
import { privateSession } from '../../../../src/main/native-contexts/index.ts';

/** A fresh fake session per partition, recording its cleanup. */
function fakeSession(name: string, calls: string[], handlers: Array<[string, unknown]> = []): any {
  return {
    name,
    setPermissionRequestHandler() {},
    setPermissionCheckHandler() {},
    setProxy: async () => {},
    on: (name: string, handler: unknown) => handlers.push([name, handler]),
    closeAllConnections: async () => calls.push('connections'),
    clearStorageData: async () => calls.push('storage'),
    clearCache: async () => calls.push('cache'),
  };
}

/** Ungoverned desktop seams with a recorded partition and protection. */
function setup() {
  const calls: string[] = [];
  const handlers: Array<[string, unknown]> = [];
  const partitions: Array<[string, unknown]> = [];
  const protectedSessions: unknown[] = [];
  const deps: any = {
    nativeBrowsing: true,
    protection: { configureSession: (session: unknown) => protectedSessions.push(session) },
    persona: { active: null },
    governance: { configuration: null, install() {} },
    externalApps: { request: () => false },
    electron: {
      app: {},
      session: {
        fromPartition: (name: string, options: unknown) => (
          partitions.push([name, options]),
          fakeSession(name, calls, handlers)
        ),
      },
    },
  };
  return { incognito: new IncognitoSession(deps), deps, calls, handlers, partitions, protectedSessions };
}

test('opens a protected in-memory partition that history, restore and agents treat as private', async () => {
  const f = setup();
  const session = await f.incognito.get();
  const [[name, options]] = f.partitions;
  assert.match(name, /^oya-incognito-/);
  assert.deepEqual(options, { cache: false });
  assert.deepEqual(f.protectedSessions, [session]);
  assert.equal(isIncognito(session), true);
  assert.equal(privateSession(session), true);
});

test('concurrent windows share one session', async () => {
  const f = setup();
  const [a, b] = await Promise.all([f.incognito.get(), f.incognito.get()]);
  assert.equal(a, b);
  assert.equal(f.partitions.length, 1);
});

test('a persona change during setup is refused, and the next window tries again', async () => {
  const f = setup();
  const pending = f.incognito.get();
  f.deps.persona.active = { id: 'replacement' };
  await assert.rejects(pending, /changed during session setup/);
  assert.deepEqual(f.protectedSessions, []);
  await f.incognito.get();
  assert.equal(f.partitions.length, 2);
});

test('retiring clears connections, storage and cache, and the next window gets a fresh session', async () => {
  const f = setup();
  const first = await f.incognito.get();
  await f.incognito.retire();
  assert.deepEqual(f.calls, ['connections', 'storage', 'cache']);
  assert.notEqual(await f.incognito.get(), first);
});

test('a window still waiting when the session retires is refused', async () => {
  const f = setup();
  const pending = f.incognito.get();
  void f.incognito.retire();
  await assert.rejects(pending, /closed while the window opened/);
});

test('refuses every download, so nothing is written to disk', async () => {
  const f = setup();
  await f.incognito.get();
  const [[name, handler]] = f.handlers.filter(([event]) => event === 'will-download');
  const calls: string[] = [];
  (handler as any)({ preventDefault: () => calls.push('prevented') }, { cancel: () => calls.push('cancelled') });
  assert.deepEqual([name, calls], ['will-download', ['prevented', 'cancelled']]);
});

test('a failed open does not drop the session a later window opened meanwhile', async () => {
  const f = setup();
  const first = f.incognito.get();
  f.deps.persona.active = { id: 'replacement' };
  void f.incognito.retire();
  const second = f.incognito.get();
  await assert.rejects(first, /changed during session setup/);
  assert.equal(await second, await f.incognito.get(), 'the later session stays the shared one');
});

test('a governed browser offers no incognito window', async () => {
  const f = setup();
  f.deps.governance.configuration = { proxy: { host: 'egress' } };
  assert.equal(f.incognito.available(), false);
  await assert.rejects(f.incognito.get(), /not available/);
  assert.equal(f.partitions.length, 0);
});

test('the headless cloud runtime offers no incognito window', (t) => {
  const f = setup();
  const before = process.env.OYA_DOCKER;
  t.after(() => (before === undefined ? delete process.env.OYA_DOCKER : (process.env.OYA_DOCKER = before)));
  process.env.OYA_DOCKER = 'true';
  assert.equal(f.incognito.available(), false);
});
