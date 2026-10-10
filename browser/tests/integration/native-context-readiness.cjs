/** Real private Oya sessions cannot be exposed while policy is pending or survive late setup after revocation. */
const assert = require('node:assert/strict');
const identity = require('../support/native-policy-identity.cjs');
const { NativeContexts, privateSession } = require('../../src/main/native-contexts/index.ts');
const { NativeSessionPolicies } = require('../../src/main/native-policy/index.ts');
/** Advance asynchronous setup deterministically without sleeps or inspector instrumentation. */
function gate() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
/** Use actual native session objects and close only exact-session windows. */
function owner(electron, configure) {
  const windows = [],
    sessions = [],
    ids = [];
  const contexts = new NativeContexts({
    create(partition) {
      const session = electron.session.fromPartition(partition, { cache: false });
      sessions.push(session);
      ids.push(partition.slice('oya-native-context-'.length));
      return session;
    },
    configure,
    close(session) {
      for (const window of windows)
        if (!window.isDestroyed() && window.webContents.session === session) window.destroy();
    },
  });
  return { contexts, sessions, ids, windows };
}
/** No first script can run through a pending context; a successful setup publishes its exact native session. */
async function configured(electron) {
  const paused = gate(),
    policies = new NativeSessionPolicies();
  const fixture = owner(electron, async (session) => {
    await paused.promise;
    policies.configure(session, {
      ...identity(),
      platform: 'Win32',
      timeZone: 'UTC',
      locale: 'de-DE',
      languages: ['de-DE', 'de'],
      hardwareConcurrency: 3,
    });
  });
  const created = fixture.contexts.create(),
    session = fixture.sessions[0],
    id = fixture.ids[0];
  try {
    assert.deepEqual(fixture.contexts.list(), []);
    assert.throws(() => fixture.contexts.get(id), /Unknown or foreign/);
    assert.equal(fixture.contexts.id(session), undefined);
    assert.equal(fixture.contexts.visible(session), false);
    assert.equal(privateSession(session), true);
    assert.equal(session._getOyaSessionPolicy().rendererStarted, false);
    paused.resolve();
    assert.equal(await created, id);
    const window = new electron.BrowserWindow({
      show: false,
      webPreferences: {
        session: fixture.contexts.get(id),
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
      },
    });
    fixture.windows.push(window);
    Object.defineProperty(window.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    const script =
      'globalThis.first={ua:navigator.userAgent,metadata:navigator.userAgentData?.toJSON(),platform:navigator.platform,zone:Intl.DateTimeFormat().resolvedOptions().timeZone,locale:Intl.NumberFormat().resolvedOptions().locale,languages:[...navigator.languages],cores:navigator.hardwareConcurrency}';
    await window.loadURL('data:text/html,' + encodeURIComponent('<!doctype html><script>' + script + '</script>'));
    assert.deepEqual(await window.webContents.executeJavaScript('first'), {
      platform: 'Win32',
      zone: 'UTC',
      locale: 'de-DE',
      languages: ['de-DE', 'de'],
      cores: 3,
      ua: identity().userAgent,
      metadata: undefined,
    });
    await fixture.contexts.remove(id);
    assert.ok(window.isDestroyed());
    assert.equal(fixture.contexts.visible(session), false);
    assert.equal(fixture.contexts.id(session), undefined);
  } finally {
    paused.resolve();
    await created.catch(() => {});
    await fixture.contexts.dispose();
  }
}
/** Native cookies created by late asynchronous setup are cleared again without republishing the context. */
async function revoked(electron, disconnect) {
  const entered = gate(),
    resume = gate(),
    url = 'http://127.0.0.1/';
  const fixture = owner(electron, async (session) => {
    await session.cookies.set({ url, name: 'before', value: 'private' });
    entered.resolve();
    await resume.promise;
    await session.cookies.set({ url, name: 'late', value: 'private' });
  });
  const rejected = assert.rejects(fixture.contexts.create(), /disconnected|cancelled/);
  const session = fixture.sessions[0],
    id = fixture.ids[0];
  try {
    await entered.promise;
    assert.equal((await session.cookies.get({ url })).length, 1);
    if (disconnect) await fixture.contexts.dispose();
    else await fixture.contexts.remove(id);
    assert.deepEqual(await session.cookies.get({ url }), []);
    resume.resolve();
    await rejected;
    assert.deepEqual(await session.cookies.get({ url }), []);
    assert.deepEqual(fixture.contexts.list(), []);
    assert.equal(fixture.contexts.id(session), undefined);
    assert.equal(fixture.contexts.visible(session), false);
    assert.equal(privateSession(session), true);
  } finally {
    resume.resolve();
    await rejected;
    await fixture.contexts.dispose();
  }
}
/** A failed identity setter retires the unpublished partition and clears its sensitive state. */
async function installationFailure(electron, stage) {
  const policies = new NativeSessionPolicies();
  const fixture = owner(electron, async (session) => {
    if (stage === 'platform') session._setOyaPlatform('MacIntel');
    else {
      session._setOyaUserAgent(identity().userAgent);
      session._setOyaUserAgentMetadata({ ...identity().userAgentMetadata, fullVersion: '9.0.0.0' });
    }
    await session.cookies.set({ url: 'http://127.0.0.1/', name: 'partial', value: 'private' });
    policies.configure(session, {
      ...identity(),
      timeZone: 'UTC',
      locale: 'de-DE',
      languages: ['de-DE', 'de'],
      hardwareConcurrency: 3,
      platform: 'Win32',
    });
  });
  try {
    await assert.rejects(fixture.contexts.create(), /installation failed/);
    const session = fixture.sessions[0];
    assert.deepEqual(fixture.contexts.list(), []);
    assert.throws(() => fixture.contexts.get(fixture.ids[0]), /Unknown or foreign/);
    assert.equal(fixture.contexts.visible(session), false);
    assert.equal(fixture.contexts.id(session), undefined);
    assert.deepEqual(await session.cookies.get({}), []);
    assert.equal(session._getOyaSessionPolicy().platform, stage === 'platform' ? 'MacIntel' : 'Win32');
    if (stage === 'metadata') assert.equal(session._getOyaSessionPolicy().userAgentMetadata.fullVersion, '9.0.0.0');
    assert.equal(session._getOyaSessionPolicy().locale, 'de-DE');
    assert.equal(session._getOyaSessionPolicy().rendererStarted, false);
    assert.throws(() => policies.assertConfigured(session), /retire this session/);
  } finally {
    await fixture.contexts.dispose();
  }
}
/** Run native readiness and both cancellation paths before broader protocol/context coverage. */
module.exports = async function nativeContextReadiness(electron) {
  await configured(electron);
  await installationFailure(electron, 'platform');
  await installationFailure(electron, 'metadata');
  await revoked(electron, false);
  await revoked(electron, true);
  console.log(
    'PASS native context readiness: unpublished pending sessions, protected first script, cancellation/disconnect fencing and real late-cookie cleanup',
  );
};
