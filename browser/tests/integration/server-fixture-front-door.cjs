/** External compatibility requests terminate in AppNativeBackend; every fixture window forbids engine debugger access. */
const { once } = require('node:events');
const { randomBytes } = require('node:crypto');
const { BrowserWindow } = require('electron');
const { WindowTabEvents } = require('../../src/main/windows/tab-events.ts');
const { AppNativeBackend } = require('../../src/main/app/native-cdp.ts');
const { startNativeFrontDoor } = require('../../src/main/native-front-door/index.ts');
/** Owned native tabs behind an ephemeral, bearer-authenticated loopback listener. */
module.exports = async function fixtureFrontDoor(window, local, actions) {
  const entries = new Map();
  const tabEvents = new WindowTabEvents();
  let nextId = 0;
  const add = (owned) => {
    const tab = {
      id: ++nextId,
      home: false,
      protection: 'protected',
      view: { webContents: owned.webContents },
      window: owned,
    };
    entries.set(tab.id, tab);
    owned.once('closed', () => {
      entries.delete(tab.id);
      tabEvents.changed();
    });
    owned.webContents.on('did-finish-load', () => tabEvents.changed());
    return tab;
  };
  add(window);
  const tabs = {
    activateTab(id) {
      entries.get(id)?.window.focus();
    },
    closeTab(id) {
      entries.get(id)?.window.destroy();
    },
    openForAutomation(url) {
      if (!local(url)) throw Error('Native fixture tabs are restricted to loopback');
      const owned = new BrowserWindow({
        show: true,
        webPreferences: {
          session: window.webContents.session,
          sandbox: true,
          contextIsolation: true,
          nodeIntegration: false,
        },
      });
      Object.defineProperty(owned.webContents, 'debugger', {
        get() {
          throw Error('Internal CDP forbidden');
        },
      });
      const tab = add(owned);
      tab.ready = owned.webContents.loadURL(url);
      return tab.id;
    },
  };
  const backend = new AppNativeBackend({
    nativeBrowsing: true,
    actions,
    windows: {
      allTabs: () => [...entries.values()],
      tabEvents,
      owner: (id) => (entries.has(id) ? { shell: { window: entries.get(id).window }, tabs } : undefined),
    },
    tabs,
    control: { busy: false, localHeld: false, connected: false, state: { mode: 'offline' } },
    governance: { allowed: local },
  });
  const token = randomBytes(32).toString('hex');
  const door = startNativeFrontDoor({ port: 0, token, backend, beginCommand: () => () => {}, clientChanged() {} });
  await once(door, 'listening');
  return { url: `ws://127.0.0.1:${door.address().port}/devtools/browser`, token };
};
