/** Native persona/cookie capture for the hermetic server login journey; only synthetic fixture credentials. */
const { Persona } = require('../../src/main/app/persona.ts');
const { CookieSync } = require('../../src/main/sync/cookie-sync.ts');
const { captureProfile } = require('../../src/main/connection/profile-capture.ts');
const { Protection } = require('../../src/main/tabs/protection.ts');
const { recordingPreferences } = require('../../src/main/recording/preload.ts');
const path = require('node:path');
/** Initialize the exact authenticated partition before creating its first page. */
module.exports = async function profileFixture(electron, auth, local) {
  const sent = [];
  const jar = electron.session.fromPartition(`persist:oya-${auth.fingerprint.id}`);
  const deps = {
    electron,
    nativeBrowsing: true,
    socket: {
      ready: false,
      ws: {},
      isOpen: () => true,
      send: (message) => {
        sent.push(message);
        return true;
      },
    },
    shell: { send() {} },
  };
  deps.persona = new Persona(deps);
  deps.cookies = new CookieSync({
    session: () => jar,
    ready: () => deps.socket.ready,
    open: () => true,
    send: (message) => deps.socket.send(message),
  });
  deps.cookies.startCookieChangeListener();
  try {
    await deps.persona.ensureLoginState(auth);
    deps.persona.active = auth.fingerprint;
    if (auth.nativePersona) {
      const protection = new Protection({
        persona: deps.persona,
        governance: { configuration: {} },
        config: { values: { provider: 'oya-cloud' } },
      });
      protection.configureSession(jar);
    }
    await deps.cookies.applyCookieSync(auth.cookies, { now: auth.now });
    jar.webRequest.onBeforeRequest((details, reply) => reply({ cancel: !local(details.url) }));
    const window = new electron.BrowserWindow({
      show: false,
      webPreferences: { ...recordingPreferences(path.resolve(__dirname, '../..')), session: jar },
    });
    Object.defineProperty(window.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    deps.socket.ready = true;
    return {
      window,
      capture: async (id) => {
        await captureProfile(deps, id);
        return sent.splice(0);
      },
      cookies: () => jar.cookies.get({}),
      dispose: () => deps.persona.disposeStorage(),
    };
  } catch (error) {
    deps.persona.disposeStorage();
    throw error;
  }
};
