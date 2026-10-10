/** Real application Persona wiring restores a cold session before native tabs and observes popup logout. */
const assert = require('node:assert/strict');
const { Persona } = require('../../src/main/app/persona.ts');
/** Use synthetic profile state only; debugger getters make hidden protocol dependencies fatal. */
module.exports = async function personaStorageChecks(electron, origin, mutation) {
  const sent = [],
    notices = [];
  const deps = {
    electron,
    nativeBrowsing: true,
    socket: {
      ready: false,
      isOpen: () => true,
      send: (message) => {
        sent.push(message);
        return true;
      },
    },
    shell: { send: (name, value) => notices.push({ name, value }) },
  };
  const persona = new Persona(deps);
  let win;
  try {
    await persona.ensureLoginState({
      fingerprint: { id: 'native-lifecycle' },
      origins: { [origin]: { account: 'Native Persona' } },
    });
    const jar = electron.session.fromPartition('persist:oya-native-lifecycle');
    win = new electron.BrowserWindow({
      show: false,
      webPreferences: { session: jar, sandbox: true, contextIsolation: true },
    });
    Object.defineProperty(win.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    await win.webContents.loadURL(origin);
    assert.equal(win.webContents.getTitle(), 'Native Persona');
    assert.equal(
      Object.hasOwn(persona, 'loginState'),
      false,
      'native profile initialization never constructs the CDP LoginState transport',
    );
    await mutation(jar, origin, () => win.webContents.executeJavaScript('localStorage.clear()'));
    assert.equal(await persona.flushStorage(), false);
    deps.socket.ready = true;
    await persona.ensureLoginState({
      fingerprint: { id: 'native-lifecycle' },
      origins: { [origin]: { account: 'Stale server copy' } },
    });
    assert.equal(await persona.flushStorage(), true);
    assert.deepEqual(sent.at(-1), { type: 'storage_changed', origins: { [origin]: {} } });
    assert.deepEqual(notices, []);
    console.log('PASS: production Persona native hydration, popup discovery, offline logout and reconnect');
  } finally {
    persona.disposeStorage();
    win?.destroy();
  }
};
