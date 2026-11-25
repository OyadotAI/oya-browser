/**
 * Real Oya permission callbacks for website notifications in a real Electron tab: the box is
 * asked once, Allow holds in a background tab and survives a reload, Block is remembered, and
 * a tab an agent drives is refused. Only the person's answer is faked; pages are served
 * in-session, so there is no network.
 */
const assert = require('node:assert/strict');
const path = require('node:path');
const { app, BrowserWindow, session } = require('electron');
const { MediaPermissions } = require('../../src/main/app/media-permissions.ts');
const { NotificationPermissions } = require('../../src/main/app/notification-permissions.ts');
const { SitePermissionStore } = require('../../src/main/app/site-permissions.ts');
const { JsonFile } = require('../../src/main/app/json-file.ts');
const { installPermissions } = require('../../src/main/identity/permissions.ts');

const profile = process.env.OYA_DESKTOP_NOTIFICATIONS_PROFILE;
if (!profile) throw new Error('Run desktop-notifications.mjs so the parent owns profile cleanup');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => app.exit(1), 30000);

/** What the page reads and gets: the permission before and after asking, then after creating a notification. */
function ask(contents) {
  return contents.executeJavaScript(
    `(async () => {
      const before = Notification.permission;
      const asked = await Notification.requestPermission();
      new Notification('Oya test', { body: 'hello', silent: true });
      await new Promise((resolve) => setTimeout(resolve, 300));
      return { before, asked, after: Notification.permission };
    })()`,
    true,
  );
}

/** Shows a notification through the page's service worker, as many web apps do; answers 'shown' or the error's name. */
function workerNotify(contents) {
  return contents.executeJavaScript(
    `(async () => {
      const registration = await navigator.serviceWorker.register('/sw.js');
      await navigator.serviceWorker.ready;
      try {
        await registration.showNotification('Oya test', { body: 'from the worker', silent: true });
        return 'shown';
      } catch (error) { return error.name; }
    })()`,
    true,
  );
}

/** Production services around a real tab; `state` is the person's answer, who holds the tab, and what is in front. */
function services(window, state) {
  const prompts = [];
  const deps = {
    electron: {
      dialog: { showMessageBox: async (_w, options) => (prompts.push(options), { response: state.response }) },
      systemPreferences: { getMediaAccessStatus: () => 'granted', askForMediaAccess: async () => true },
      shell: { openExternal: async () => undefined },
    },
    shell: { window },
    tabs: {
      activeTabId: 1,
      list: [{ id: 1, view: { webContents: window.webContents } }],
      getActiveView: () => (state.background ? { webContents: null } : { webContents: window.webContents }),
    },
    agentTab: { humanMay: () => state.human },
    governance: { configuration: null },
    persona: { partitionName: () => 'persist:notifications-test' },
    sitePermissions: new SitePermissionStore(new JsonFile(() => path.join(profile, 'site-permissions.json'))),
  };
  deps.mediaPermissions = new MediaPermissions(deps, 'darwin');
  return { prompts, notifications: new NotificationPermissions(deps) };
}

/** Allow once, then a reload, a background tab and an agent taking the tab. */
async function allowed(ses, window, state) {
  const f = services(window, state);
  installPermissions(ses, undefined, undefined, undefined, f.notifications);
  await window.loadURL('https://mail.test/inbox');
  assert.deepEqual(await ask(window.webContents), { before: 'denied', asked: 'granted', after: 'granted' });
  assert.equal(f.prompts.filter((p) => p.message === 'Show notifications?').length, 1);
  await window.loadURL('https://mail.test/other');
  state.background = true;
  assert.deepEqual(await ask(window.webContents), { before: 'granted', asked: 'granted', after: 'granted' });
  assert.equal(f.prompts.length, 1, 'a remembered Allow, in a background tab, asks nothing');
  assert.equal(await workerNotify(window.webContents), 'shown', 'a service worker of an allowed, open site');
  state.human = false;
  assert.deepEqual(await ask(window.webContents), { before: 'denied', asked: 'denied', after: 'denied' });
  assert.equal(await workerNotify(window.webContents), 'TypeError', 'not while an agent holds the only tab');
  Object.assign(state, { background: false, human: true });
}

/** Block once; the reloaded page is refused without another box. */
async function blocked(ses, window, state) {
  const f = services(window, { ...state, response: 0 });
  installPermissions(ses, undefined, undefined, undefined, f.notifications);
  await window.loadURL('https://news.test/');
  assert.equal((await ask(window.webContents)).asked, 'denied');
  await window.loadURL('https://news.test/again');
  assert.equal((await ask(window.webContents)).asked, 'denied');
  assert.equal(f.prompts.length, 1);
}

/** Serves every https page in-session, so nothing leaves the machine. */
function servePages(ses) {
  ses.protocol.handle('https', (request) =>
    new URL(request.url).pathname === '/sw.js'
      ? new Response('self.addEventListener("install", () => self.skipWaiting());', {
          headers: { 'content-type': 'text/javascript' },
        })
      : new Response('<!doctype html><title>Inbox</title><p>Inbox</p>', { headers: { 'content-type': 'text/html' } }),
  );
}

/** One window, one profile: the Allow paths, then the Block path; exits 0 only if every check held. */
async function run() {
  await app.whenReady();
  const ses = session.fromPartition('persist:notifications-test');
  servePages(ses);
  const window = new BrowserWindow({ show: false, webPreferences: { session: ses, sandbox: true } });
  const state = { response: 1, human: true, background: false };
  await allowed(ses, window, state);
  await blocked(ses, window, state);
  window.destroy();
  clearTimeout(deadline);
  console.log('desktop notifications: ok');
  app.exit(0);
}

run().catch((error) => {
  console.error(error);
  app.exit(1);
});
