/** Persona storage coordinates cold initialization, native surfaces, reconnect and identity-safe delivery. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { PersonaStorage } from '../../../../src/main/sync/persona-storage.ts';
import { STORAGE_FLUSH_MS } from '../../../../src/main/sync/constants.ts';
const origin = 'https://example.test';
/** Drain asynchronous native replies without real timers. */
async function settle() {
  for (let i = 0; i < 60; i++) await Promise.resolve();
}
/** Per-session native store; no shared mutable current-jar getter. */
function jar() {
  const session: any = new EventEmitter();
  session.values = new Map<string, string[][]>();
  session.restored = [];
  session.reads = 0;
  session._restoreOyaLocalStorage = async (origin, entries) => {
    session.restored.push(origin);
    if (!session.values.get(origin)?.length) session.values.set(origin, entries);
    return entries;
  };
  session._readOyaLocalStorage = async (origin) => {
    session.reads++;
    return session.values.get(origin) || [];
  };
  session._watchOyaLocalStorage = async () => {};
  session._unwatchOyaLocalStorage = () => {};
  return session;
}
/** An Electron-style native surface with committed top-level navigations. */
function surface(session, url = 'about:blank') {
  const contents: any = new EventEmitter();
  Object.assign(contents, { session, isDestroyed: () => false, getURL: () => url });
  return contents;
}
/** Capture transport publication and visible failures while controlling connectivity. */
function fixture(contents: any[] = []) {
  const app: any = new EventEmitter(),
    sent: any[] = [],
    errors: string[] = [];
  const link = { online: false, accepted: true };
  const service = new PersonaStorage({
    app,
    contents: () => contents,
    online: () => link.online,
    send: (message) => {
      if (link.accepted) sent.push(message);
      return link.accepted;
    },
    report: (error) => errors.push(error),
  });
  return { app, sent, errors, link, service };
}
it('hydrates a cold authenticated session before any surface is created', async () => {
  const f = fixture(),
    session = jar();
  try {
    await f.service.activate(session, { [origin]: { account: 'Alice' } });
    assert.deepEqual(session.restored, [origin]);
    assert.deepEqual(session.values.get(origin), [['account', 'Alice']]);
    assert.equal(f.sent.length, 0);
    f.link.online = true;
    assert.equal(await f.service.flush(), true);
    assert.equal(f.sent[0].origins[origin].account, 'Alice');
  } finally {
    f.service.dispose();
  }
});
it('never restores over an exposed empty partition or repeats imports on reconnect', async () => {
  const session = jar(),
    page = surface(session, origin),
    f = fixture([page]);
  try {
    await f.service.activate(session, { [origin]: { account: 'stale' } });
    f.link.online = true;
    await f.service.flush();
    assert.deepEqual(f.sent[0].origins[origin], {});
    await f.service.activate(session, { [origin]: { account: 'stale-again' } });
    assert.deepEqual(session.restored, []);
  } finally {
    f.service.dispose();
  }
});
it('discovers native popup origins, ignores subframes, and retries offline clears on reconnect', async () => {
  const f = fixture(),
    session = jar(),
    popup = surface(session);
  try {
    await f.service.activate(session, {});
    f.app.emit('web-contents-created', {}, popup);
    session.values.set(origin, [['account', 'Alice']]);
    popup.emit('did-navigate', {}, origin + '/login');
    popup.emit('did-navigate-in-page', {}, 'https://third-party.test/frame', false);
    await settle();
    assert.equal(await f.service.flush(), false);
    session.values.set(origin, []);
    session.emit('oya-local-storage-changed', {}, origin, true);
    f.link.online = true;
    await f.service.activate(session, { [origin]: { account: 'stale' } });
    await f.service.flush();
    assert.deepEqual(f.sent, [{ type: 'storage_changed', origins: { [origin]: {} } }]);
    assert.deepEqual(session.restored, []);
  } finally {
    f.service.dispose();
  }
  assert.equal(popup.listenerCount('did-navigate'), 0);
});
it('retiring a persona never sends its queued values through the next identity', async () => {
  const f = fixture(),
    a = jar(),
    b = jar();
  try {
    await f.service.activate(a, { [origin]: { account: 'Alice' } });
    await f.service.activate(b, { [origin]: { account: 'Bob' } });
    f.link.online = true;
    a.emit('oya-local-storage-changed', {}, origin, true);
    await f.service.flush();
    assert.deepEqual(f.sent, [{ type: 'storage_changed', origins: { [origin]: { account: 'Bob' } } }]);
    assert.equal(a.listenerCount('oya-local-storage-changed'), 0);
    a.values.set(origin, []);
    await f.service.activate(a, { [origin]: { account: 'stale-Alice' } });
    await f.service.flush();
    assert.deepEqual(f.sent.at(-1).origins[origin], {});
    assert.equal(a.restored.length, 1);
  } finally {
    f.service.dispose();
  }
});
it('refuses a page exposed during cold hydration rather than claiming readiness', async () => {
  const f = fixture(),
    session = jar();
  let finish;
  session._restoreOyaLocalStorage = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  try {
    const pending = f.service.activate(session, { [origin]: {} });
    await settle();
    f.app.emit('web-contents-created', {}, surface(session));
    finish([]);
    await assert.rejects(pending, /disposed/);
    await assert.rejects(f.service.flush(), /disposed/);
    assert.equal(f.sent.length, 0);
    assert.equal(f.errors.length, 1);
  } finally {
    f.service.dispose();
  }
});
it('batches native mutations without rescanning unchanged origins', async (t) => {
  t.mock.timers.enable({ apis: ['setInterval'] });
  const f = fixture(),
    session = jar();
  try {
    await f.service.activate(session, { [origin]: { account: 'Alice' } });
    f.link.online = true;
    const reads = session.reads;
    t.mock.timers.tick(STORAGE_FLUSH_MS);
    await settle();
    t.mock.timers.tick(STORAGE_FLUSH_MS);
    await settle();
    assert.equal(f.sent.length, 1);
    assert.equal(session.reads, reads);
    session.values.set(origin, []);
    session.emit('oya-local-storage-changed', {}, origin, true);
    await settle();
    t.mock.timers.tick(STORAGE_FLUSH_MS);
    await settle();
    assert.deepEqual(f.sent.at(-1).origins[origin], {});
  } finally {
    f.service.dispose();
  }
});
it('validates before retiring the current identity and removes application listeners on disposal', async () => {
  const f = fixture(),
    a = jar(),
    b = jar();
  await f.service.activate(a, {});
  await assert.rejects(f.service.activate(b, { 'file:///tmp': {} }), /origin/);
  assert.equal(a.listenerCount('oya-local-storage-changed'), 1);
  f.service.dispose();
  f.service.dispose();
  assert.equal(f.app.listenerCount('web-contents-created'), 0);
  await assert.rejects(f.service.activate(a, {}), /disposed/);
});

it('even an empty cold import refuses readiness after premature surface exposure', async () => {
  const f = fixture(),
    session = jar();
  const pending = f.service.activate(session, {});
  f.app.emit('web-contents-created', {}, surface(session));
  await assert.rejects(pending, /interrupted/);
  assert.equal(f.errors.length, 1);
  f.service.dispose();
});
