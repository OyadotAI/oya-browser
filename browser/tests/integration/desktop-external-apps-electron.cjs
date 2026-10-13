/** Real native navigation, redirects and popup interception; OS app launches and user choices are fake. */
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { setTimeout: pause } = require('node:timers/promises');
const { app, BrowserWindow } = require('electron');
const { ExternalApps } = require('../../src/main/app/external-apps.ts');
const { TabWindows } = require('../../src/main/tabs/tab-windows.ts');
const { NavigationHandlers } = require('../../src/main/ipc/navigation.ts');
const { installPermissions } = require('../../src/main/identity/permissions.ts');

if (!process.env.OYA_EXTERNAL_APPS_PROFILE) throw new Error('Run desktop-external-apps.mjs');
app.setPath('userData', process.env.OYA_EXTERNAL_APPS_PROFILE);
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('External apps test timed out');
  app.exit(1);
}, 45000);
const links = {
  docker: 'docker-desktop://auth/callback?code=private%2Fcode+value&state=example#done',
  slack: 'slack://channel?team=T123&id=C456',
  zoom: 'zoommtg://zoom.us/join?confno=123&pwd=private',
  zoomus: 'zoomus://zoom.us/start?confno=123',
  other: 'example-editor://open?token=private',
};
const page = `<html><body style="padding:30px;font:20px sans-serif">${Object.entries(links)
  .map(([id, url]) => `<p><a id="${id}" href="${url}">${id}</a></p>`)
  .join('')}
  <p><a id="popup" target="app-window" href="${links.slack}">Popup</a></p>
  <p><a id="redirect" href="/redirect">Redirect</a></p></body></html>`;
const server = createServer((req, res) => {
  if (req.url === '/redirect') {
    res.writeHead(302, { Location: links.docker });
    return res.end();
  }
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(page);
});
const state = { human: true, response: 1, pending: null, missing: false };
const opened = [],
  prompts = [],
  lookups = [];

/** Poll a native event with a bounded deadline, rather than assuming renderer timing. */
async function until(check, description) {
  const end = Date.now() + 5000;
  while (!check()) {
    if (Date.now() > end) throw new Error(description);
    await pause(20);
  }
}
/** A trusted pointer click in the Oya renderer, without any debugger protocol. */
async function click(wc, id) {
  const point = await wc.executeJavaScript(
    `(() => { const r=document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return { x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2) }; })()`,
  );
  wc.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, ...point });
  wc.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, ...point });
}
/** Use the production window-open and permission routers around actual native navigation. */
function setup(window) {
  const wc = window.webContents;
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw new Error('Debugger access is forbidden');
    },
  });
  const deps = {
    electron: {
      app: {
        getApplicationInfoForProtocol: async (url) => {
          lookups.push(url);
          if (state.missing) throw new Error('No handler');
          return { name: url.startsWith('docker-') ? 'Docker' : url.startsWith('slack:') ? 'Slack' : 'Desktop app' };
        },
      },
      dialog: {
        showMessageBox: async (parent, options) => {
          assert.equal(parent, window);
          prompts.push(options);
          return state.pending ? state.pending.promise : { response: state.response };
        },
      },
      shell: {
        openExternal: async (url) => {
          opened.push(url);
        },
      },
    },
    shell: { window, browsingMode: true },
    tabs: {
      activeTabId: 1,
      getActiveView: () => ({ webContents: wc }),
      navigateActive: () => assert.fail('app link became a page'),
      createTab: () => assert.fail('app link created an empty tab'),
    },
    agentTab: { humanMay: () => state.human, requireFor: () => assert.ok(state.human) },
    governance: { configuration: null },
    persona: { partitionName: () => 'persist:app-links' },
    control: { snapshot: () => ({ interactive: false, mode: 'agent', connected: true }) },
    deepLinks: { applyDeepLink: () => assert.fail('app link became pairing') },
  };
  deps.externalApps = new ExternalApps(deps);
  new TabWindows(deps).wire({ id: 1, view: { webContents: wc } });
  installPermissions(wc.session, (url, contents) => deps.externalApps.request(url, contents));
  return deps;
}
/** Every entry path launches exactly once, and the page remains mounted and at its original URL. */
async function entryPaths(window, deps, url) {
  const wc = window.webContents;
  for (const id of Object.keys(links).concat(['popup', 'redirect'])) {
    const before = opened.length,
      asked = prompts.length;
    await click(wc, id);
    await until(() => opened.length > before, `No handoff for ${id}; prompts=${JSON.stringify(prompts.slice(asked))}`);
    await pause(100);
    assert.equal(opened.length, before + 1, 'one gesture must launch exactly once');
    assert.equal(opened.at(-1), links[id] || (id === 'popup' ? links.slack : links.docker));
    assert.equal(wc.getURL(), url);
    assert.equal(BrowserWindow.getAllWindows().length, 1);
  }
  const before = opened.length;
  await new NavigationHandlers(deps).handlers.navigate({}, links.docker);
  await until(() => opened.length === before + 1, 'pasted callback did not launch');
  assert.equal(opened.at(-1), links.docker);
  assert.ok(prompts.every((p) => p.defaultId === 0 && p.cancelId === 0));
  assert.doesNotMatch(JSON.stringify(prompts), /private|confno|T123|C456/);
}
/** Cancellation, absence and ownership changes must never become an OS launch. */
async function refusals(window, deps, url) {
  const wc = window.webContents,
    before = opened.length;
  state.response = 0;
  await click(wc, 'slack');
  await pause(100);
  assert.equal(opened.length, before);
  state.response = 1;
  state.human = false;
  const asked = prompts.length;
  await click(wc, 'docker');
  await pause(100);
  assert.equal(prompts.length, asked);
  state.human = true;
  state.missing = true;
  await click(wc, 'slack');
  await until(() => prompts.length > asked, 'missing-app explanation absent');
  assert.equal(prompts.at(-1).message, 'No app can open this link');
  state.missing = false;
  state.pending = Promise.withResolvers();
  const waiting = prompts.length;
  await click(wc, 'zoom');
  await until(() => prompts.length > waiting, 'confirmation absent');
  await wc.loadURL(url);
  state.pending.resolve({ response: 1 });
  state.pending = null;
  await pause(100);
  assert.equal(opened.length, before, 'same-URL reload must cancel consent');
  assert.equal(wc.getURL(), url);
}
/** Run against Oya, with only local fixture traffic and no real app launches. */
async function run() {
  await app.whenReady();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}/`;
  const window = new BrowserWindow({
    width: 900,
    height: 720,
    show: true,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  const deps = setup(window);
  await window.loadURL(url);
  window.focus();
  await window.webContents.executeJavaScript(
    'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))',
  );
  await entryPaths(window, deps, url);
  await refusals(window, deps, url);
  console.log(
    `Oya desktop app links passed: ${opened.length} handoffs, native clicks, redirects, popup and address entry, cancellation, missing handler, ownership and reload fences.`,
  );
  window.destroy();
  await new Promise((resolve) => server.close(resolve));
  clearTimeout(deadline);
  app.exit(0);
}
void run().catch((error) => {
  console.error(error);
  clearTimeout(deadline);
  server.close();
  app.exit(1);
});
