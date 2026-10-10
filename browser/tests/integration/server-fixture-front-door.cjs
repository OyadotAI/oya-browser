/** External compatibility entry for the native SDK journey: requests terminate in AppNativeBackend, never a debugger. */
const { once } = require('node:events');
const { randomBytes } = require('node:crypto');
const { WindowTabEvents } = require('../../src/main/windows/tab-events.ts');
const { AppNativeBackend } = require('../../src/main/app/native-cdp.ts');
const { startNativeFrontDoor } = require('../../src/main/native-front-door/index.ts');
/** A single owned native tab behind an ephemeral, bearer-authenticated loopback listener. */
module.exports = async function fixtureFrontDoor(window, local) {
  const tab = { id: 1, home: false, protection: 'protected', view: { webContents: window.webContents } };
  const backend = new AppNativeBackend({
    nativeBrowsing: true,
    windows: { allTabs: () => [tab], tabEvents: new WindowTabEvents(), owner: () => ({ shell: { window } }) },
    tabs: { activateTab() {} },
    control: { busy: false, localHeld: false, connected: false, state: { mode: 'offline' } },
    governance: { allowed: local },
  });
  const token = randomBytes(32).toString('hex');
  const door = startNativeFrontDoor({ port: 0, token, backend, beginCommand: () => () => {}, clientChanged() {} });
  await once(door, 'listening');
  return { url: `ws://127.0.0.1:${door.address().port}/devtools/browser`, token };
};
