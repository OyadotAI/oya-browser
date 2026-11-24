/** Exercise exact cross-origin document tools and real native input in disposable Oya windows. */
const assert = require('node:assert/strict');
const http = require('node:http');
const path = require('node:path');
const fs = require('node:fs');
const portal = require('./fixtures/portal-widgets.cjs');
const electron = require('electron');
const { app, BrowserWindow, BrowserView, ipcMain } = electron;
const { ControlShield } = require('../../src/main/shell/control-shield.ts');
const { installPointerFocusFence } = require('../../src/main/input/index.ts');
const {
  useDebugChannel,
  useDebugApplication,
  useNativeMouse,
  inspectDebugMouseSurface,
  browserDiagnostic,
} = require('../../src/main/browser-debug/index.ts');
const { inLane } = require('../../src/main/control/agent-tab.ts');
const { STOCK_PERSONA } = require('../../src/shared/stock-persona.ts');
if (!process.env.OYA_DEBUG_PROFILE) throw Error('Parent-owned profile required');
app.setPath('userData', process.env.OYA_DEBUG_PROFILE);
app.commandLine.appendSwitch('site-per-process');
let win, view, personal, shield;
const events = `<script nonce="debug">window.clicks=0;window.events=[];for(const t of ['mousemove','mousedown','mouseup','click'])document.addEventListener(t,e=>{events.push({type:e.type,trusted:e.isTrusted,id:e.target.id});if(e.type==='click')clicks++;});</script>`;
const baseline = `<!doctype html><title>Baseline form</title><label for="n">First name</label><input id="n"><span id="l">Work email</span><input aria-labelledby="l"><details id="advanced"><summary>Advanced</summary>Extra</details><div role="option" tabindex="0">Inbox</div><div id="editor" contenteditable="plaintext-only">Draft</div><select aria-label="Plan"><option>Free</option><option>Team</option></select><input type="password" aria-label="Password" value="private-marker">${events}`;
const server = http.createServer((req, res) => {
  const port = server.address().port;
  res.setHeader('content-type', 'text/html');
  res.setHeader(
    'content-security-policy',
    "default-src 'self'; script-src 'nonce-debug'; style-src 'unsafe-inline'; frame-src http://localhost:* http://127.0.0.1:*",
  );
  if (req.url === '/baseline') return res.end(baseline);
  if (req.url === '/portal-widgets') return res.end(portal + events);
  res.end(
    req.url === '/parent'
      ? `<!doctype html><title>Owned portal</title><iframe id="left" src="http://localhost:${port}/child" style="position:absolute;left:50px;top:60px;width:300px;height:180px;border:0"></iframe><iframe id="right" src="http://localhost:${port}/child" style="position:absolute;left:400px;top:60px;width:300px;height:180px;border:0"></iframe>`
      : `<!doctype html><title>Child form</title><style>body{margin:0}button{position:absolute;left:20px;top:20px;width:100px;height:50px}input{position:absolute;left:20px;top:90px}</style><button id="target">Frame button</button><input id="name" aria-label="Name">${events}`,
  );
});
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const deadline = setTimeout(() => finish(1), 30000);
/** Exercise diagnostic success and refusal through the owned native adapter. */
async function run() {
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  ipcMain.on(STOCK_PERSONA.CHANNEL, (e) => (e.returnValue = ''));
  useDebugChannel(ipcMain);
  useDebugApplication({
    name: () => app.getName(),
    version: () => app.getVersion(),
    engine: () => process.versions.electron,
  });
  win = new BrowserWindow({
    width: 900,
    height: 650,
    show: false,
    webPreferences: { sandbox: true, contextIsolation: true },
  });
  await win.loadURL('data:text/html,<title>Owned shell fixture</title>');
  view = new BrowserView({
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      backgroundThrottling: false,
      nodeIntegrationInSubFrames: true,
      additionalArguments: [STOCK_PERSONA.ARGUMENT],
      preload: path.resolve(__dirname, '../../out/preload/recording.js'),
    },
  });
  view.setBounds({ x: 0, y: 50, width: 900, height: 600 });
  win.addBrowserView(view);
  Object.defineProperty(view.webContents, 'debugger', {
    get() {
      throw Error('CDP is forbidden');
    },
  });
  const control = { interactive: false };
  const driver = {
    deps: {
      getActiveView: () => view,
      diagnosticGuard: () => {
        if (control.interactive || win.isDestroyed() || !win.getBrowserViews().includes(view))
          throw Error('Ownership changed');
      },
    },
  };
  const tabs = {
    activeTabId: 1,
    list: [{ id: 1, view }],
    getShownView: () => view,
    find: (id) => (id === 1 ? { id: 1, view } : null),
  };
  const deps = {
    electron,
    appDir: path.resolve(__dirname, '../..'),
    control: { snapshot: () => ({ interactive: control.interactive, mode: 'agent', mine: true }) },
    shell: { window: win, browsingMode: true, send() {}, alive: () => true },
    overlays: { names: new Set() },
    agentTab: { scoped: () => true, claimed: () => [1], humanMay: (id) => id !== 1, view: () => view },
    tabs,
    shortcuts: { install() {} },
    recorder: {},
    world: {},
  };
  shield = new ControlShield(deps);
  installPointerFocusFence(
    view.webContents,
    () => control.interactive,
    () => shield.keepFocusOnShell(),
  );
  let shieldMouse = 0,
    personalMouse = 0;
  shield.prepare().webContents.on('input-event', (_event, input) => {
    if (['mouseDown', 'mouseUp', 'mouseMove', 'mouseWheel'].includes(input.type)) shieldMouse++;
  });

  useNativeMouse(
    () => ({
      window: win,
      ...inspectDebugMouseSurface(win, view, shield.view),
      activate: () => app.focus({ steal: true }),
    }),
    path.resolve(__dirname, '../../out/native/owned-pointer.node'),
  );
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/parent`);
  win.show();
  win.focus();
  app.focus({ steal: true });
  shield.sync();
  await pause(200);
  const call = (p) => inLane('mcp-debug-fixture', () => browserDiagnostic(driver, view, p));
  await call({
    kind: 'eval',
    expression:
      "(()=>{window.rootEvents=[];document.addEventListener('mousemove',e=>rootEvents.push({x:e.clientX,y:e.clientY,id:e.target.id}));return true})()",
  });
  let read = await call({ kind: 'read' });
  assert.equal(read.ok, true, read.text);
  assert.equal(read.data.read, 3, read.text);
  const children = read.data.frames.filter((f) => f.parent);
  assert.equal(children.length, 2);
  assert.notEqual(children[0].ref, children[1].ref);
  const left = children[0].ref,
    right = children[1].ref;
  const value = await call({
    kind: 'eval',
    frame: left,
    expression: '({title:document.title,node:typeof require,clicks:window.clicks})',
  });
  assert.equal(value.ok, true, value.text);
  assert.deepEqual(value.data.value, { title: 'Child form', node: 'undefined', clicks: 0 });
  let hit = await call({ kind: 'hit_test', x: 120, y: 105 });
  assert.equal(hit.ok, true, hit.text);
  assert.equal(hit.data.element.id, 'target');
  assert.equal(hit.data.frame, left);
  assert.equal(hit.data.partial, false, JSON.stringify(hit));
  /** Resolve only public element numbers from this fixture read. */
  function element(frame, label) {
    const section = read.text.split('## Frame ' + frame)[1]?.split('## Frame ')[0];
    const line = section?.split('\n').find((l) => l.includes(label) && l.startsWith('['));
    return Number(line?.match(/^\[(\d+)\]/)?.[1]);
  }
  /** Snapshot page and native focus so refusal cannot hide preparation effects. */
  async function state() {
    return {
      root: (
        await call({
          kind: 'eval',
          expression: '({scrollX,scrollY,focus:document.activeElement.id,events:rootEvents})',
        })
      ).data.value,
      left: (
        await call({
          kind: 'eval',
          frame: left,
          expression: '({scrollX,scrollY,focus:document.activeElement.id,clicks,events})',
        })
      ).data.value,
      focused: electron.webContents.getFocusedWebContents()?.id,
      active: win.isFocused(),
      stack: win.getBrowserViews().map((v) => v.webContents.id),
      shieldMouse,
      personalMouse,
    };
  }
  assert.deepEqual(
    win.getBrowserViews().map((v) => v.webContents.id),
    [view.webContents.id, shield.view.webContents.id],
    'mounted views are ordered bottom to top',
  );
  await call({ kind: 'eval', frame: left, expression: "document.getElementById('target').style.top='1000px',true" });
  read = await call({ kind: 'read' });
  const shieldBefore = await state(),
    refused = await call({ kind: 'click', element: element(left, 'Frame button'), framed: true });
  assert.equal(refused.ok, false);
  assert.equal(refused.outcome, 'native_input_unavailable');
  assert.equal(refused.data.reason, 'browser_control_shield');
  assert.deepEqual(refused.data.dispatched, []);
  assert.deepEqual(await state(), shieldBefore, 'entry refusal cannot scroll, focus, move or activate');
  assert.ok(refused.identity.version);
  read = await call({ kind: 'read' });
  assert.equal(
    (await call({ kind: 'type', element: element(left, 'Name'), text: 'Ada' })).ok,
    true,
    'independent DOM-value typing remains available',
  );
  await call({ kind: 'eval', frame: left, expression: "document.getElementById('target').style.top='20px',true" });
  const shieldBaseline = shieldMouse;
  win.removeBrowserView(shield.view); // Fixture-only: independently verify the unoccluded native route.
  await call({
    kind: 'eval',
    expression: `(()=>{const n=document.createElement('div');n.id='cover';n.style.cssText='position:fixed;left:0;top:0;right:0;bottom:0;z-index:999;background:white';document.body.append(n);return true})()`,
  });
  hit = await call({ kind: 'hit_test', x: 120, y: 105 });
  assert.equal(hit.data.element.id, 'cover');
  assert.equal(hit.data.framePath.length, 1);
  read = await call({ kind: 'read' });
  assert.equal(read.ok, true, read.text);

  const blocked = await call({ kind: 'click', element: element(left, 'Frame button'), framed: true });
  assert.equal(blocked.ok, false);
  assert.match(blocked.text, /covered/);
  await call({ kind: 'eval', expression: "document.getElementById('cover').remove(),true" });
  read = await call({ kind: 'read' });
  assert.equal(read.ok, true, read.text);
  const clicked = await call({ kind: 'click', element: element(left, 'Frame button'), framed: true });
  assert.equal(clicked.ok, true, clicked.text);
  assert.deepEqual(
    clicked.data.dispatched.filter((event) => event === 'mousedown' || event === 'mouseup'),
    ['mousedown', 'mouseup'],
  );
  assert.equal(shieldMouse, shieldBaseline, 'native command adds no shield input');
  await pause(100);
  const observed = await call({ kind: 'eval', frame: left, expression: '({clicks,events})' });
  assert.equal(observed.data.value.clicks, 1);
  assert.ok(observed.data.value.events.every((e) => e.trusted));
  assert.equal((await call({ kind: 'eval', frame: right, expression: 'clicks' })).data.value, 0);
  read = await call({ kind: 'read' });
  const typed = await call({ kind: 'type', element: element(left, 'Name'), text: 'Ada' });
  assert.equal(typed.ok, true, typed.text);
  assert.equal(
    (await call({ kind: 'eval', frame: left, expression: "document.getElementById('name').value" })).data.value,
    'Ada',
  );
  assert.equal((await call({ kind: 'eval', frame: left, expression: "0); return 'early'; (0" })).data.value, 0);
  for (const expression of [
    'undefined',
    '1n',
    'document.body',
    'Promise.resolve(1)',
    '(()=>{const x={};x.x=x;return x})()',
  ])
    assert.equal((await call({ kind: 'eval', frame: left, expression })).ok, false, expression);
  personal = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true } });
  personal.setBounds(view.getBounds());
  win.addBrowserView(personal);
  win.setTopBrowserView(personal);
  await personal.webContents.loadURL('data:text/html,<input id="personal" value="keep">');
  personal.webContents.on('input-event', (_event, input) => {
    if (['mouseDown', 'mouseUp', 'mouseMove', 'mouseWheel'].includes(input.type)) personalMouse++;
  });
  personal.webContents.focus();
  await pause(20);
  assert.equal(win.getBrowserViews().at(-1), personal, 'setTopBrowserView puts the native view last');
  read = await call({ kind: 'read' });
  const personalBefore = await state();
  const background = await call({ kind: 'click', element: element(right, 'Frame button'), framed: true });
  assert.equal(background.ok, false);
  assert.equal(background.outcome, 'native_input_unavailable');
  assert.equal(background.data.reason, 'native_occlusion');
  assert.deepEqual(background.data.dispatched, []);
  assert.deepEqual(await state(), personalBefore);
  assert.equal(personalMouse, personalBefore.personalMouse, 'refusal adds no personal input');
  assert.equal(shieldMouse, shieldBaseline);
  assert.equal(personal.webContents.isFocused(), true, 'prior native focus is restored before yielding');
  assert.equal(await personal.webContents.executeJavaScript("document.getElementById('personal').value"), 'keep');
  win.removeBrowserView(personal);
  personal.webContents.close();
  personal = null;
  await pause(20);
  await call({
    kind: 'eval',
    frame: left,
    expression: `(()=>{const host=document.createElement('div');host.id='shadow-host';host.setAttribute('role','button');host.setAttribute('aria-label','Shadow host');host.style.cssText='position:absolute;left:160px;top:20px;width:100px;height:50px';host.attachShadow({mode:'open'}).innerHTML='<button id="shadow-target" style="width:100%;height:100%">Shadow action</button>';document.body.append(host);return true})()`,
  });
  read = await call({ kind: 'read' });
  assert.equal(read.ok, true, read.text);
  assert.equal(
    read.text
      .split('\n')
      .find((line) => line.startsWith('[') && line.includes('Shadow host'))
      ?.includes('(covered)'),
    false,
  );
  const shadow = await call({ kind: 'click', element: element(left, 'Shadow action'), framed: true });
  assert.equal(shadow.ok, true, shadow.text);
  await call({ kind: 'eval', frame: left, expression: "document.getElementById('target').id='x'.repeat(300000),true" });
  const oversizedHit = await call({ kind: 'hit_test', x: 120, y: 105 });
  assert.equal(oversizedHit.ok, false);
  assert.match(oversizedHit.text, /256 KiB/);
  assert.ok(oversizedHit.identity.version);
  await call({ kind: 'eval', frame: left, expression: "document.querySelector('button').id='target',true" });
  await call({
    kind: 'eval',
    frame: left,
    expression: `(()=>{const frame=document.createElement('iframe');frame.id='nested';frame.src='http://127.0.0.1:${server.address().port}/grandchild';frame.style.cssText='position:absolute;left:150px;top:80px;width:140px;height:90px;border:0';document.body.append(frame);return true})()`,
  });
  await pause(150);
  read = await call({ kind: 'read' });
  assert.equal(read.data.read, 4, read.text);
  const grand = read.data.frames.find((frame) => frame.url.endsWith('/grandchild'));
  assert.equal(grand.parent, left);
  const deepHit = await call({ kind: 'hit_test', x: 270, y: 185 });
  assert.equal(deepHit.ok, true, deepHit.text);
  assert.equal(deepHit.data.frame, grand.ref);
  assert.equal(deepHit.data.framePath.length, 3);
  const deepClick = await call({ kind: 'click', element: element(grand.ref, 'Frame button'), framed: true });
  assert.equal(deepClick.ok, true, deepClick.text);
  assert.equal((await call({ kind: 'eval', frame: grand.ref, expression: 'clicks' })).data.value, 1);
  await call({ kind: 'eval', frame: left, expression: "document.getElementById('nested').remove(),true" });
  const detached = await call({ kind: 'eval', frame: grand.ref, expression: 'window.wrong=1' });
  assert.equal(detached.ok, false);
  const frame = view.webContents.mainFrame.frames[0];
  await frame.executeJavaScript('location.reload()');
  await pause(300);
  const stale = await call({ kind: 'eval', frame: left, expression: 'window.wrong=1' });
  assert.equal(stale.ok, false, 'same-URL reload invalidates frame reference');
  const fresh = await call({ kind: 'read' });
  const hung = fresh.data.frames.find((frame) => frame.parent).ref;
  const timeout = await call({
    kind: 'eval',
    frame: hung,
    expression: '(()=>{window.once=(window.once??0)+1;while(true){}})()',
  });
  assert.equal(timeout.ok, false);
  assert.match(timeout.text, /timed out.*may have run/i);
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/baseline`);
  read = await call({ kind: 'read', frames: false });
  assert.equal(read.ok, true, read.text);
  assert.equal(read.data.read, 1);
  const root = read.data.frames[0].ref;
  for (const name of ['First name', 'Work email', 'Advanced', 'Inbox', 'Draft', 'Plan'])
    assert.ok(element(root, name) > 0, `default read retains ${name}`);
  assert.match(read.text, /Title: Baseline form/);
  assert.match(read.text, /"options":\["Free","Team"\]/);
  assert.equal(read.text.includes('private-marker'), false);
  const disclosure = await call({ kind: 'click', element: element(root, 'Advanced'), framed: true });
  assert.equal(disclosure.ok, true, disclosure.text);
  assert.equal((await call({ kind: 'eval', expression: "document.getElementById('advanced').open" })).data.value, true);
  read = await call({ kind: 'read', frames: false });
  assert.equal((await call({ kind: 'type', element: element(root, 'Draft'), text: 'Updated draft' })).ok, true);
  assert.equal(
    (await call({ kind: 'eval', expression: "document.getElementById('editor').textContent" })).data.value,
    'Updated draft',
  );
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/portal-widgets`);
  read = await call({ kind: 'read', frames: false });
  const portalFrame = read.data.frames[0].ref;
  for (const name of [
    '23',
    '22',
    'PAYER - CA',
    'Jordan Example Provider',
    'search',
    'trash',
    'Tartine Bakery',
    'Tartine Manufactory',
  ])
    assert.ok(element(portalFrame, name) > 0, `canonical portal control ${name} retains a private number`);
  const stars = read.text.split('\n').filter((line) => /^\[\d+\] radio/.test(line));
  assert.equal(stars.length, 2);
  assert.equal(stars.filter((line) => line.includes('"checked":true')).length, 1);
  const rows = read.text.split('\n').filter((line) => /^\[\d+\]/.test(line) && line.includes('Tartine'));
  assert.equal(rows.length, 2);
  assert.ok(!read.text.split('\n').some((line) => /^\[\d+\]/.test(line) && line.includes('whole panel')));
  const starId = Number(stars[0].match(/^\[(\d+)\]/)[1]);
  assert.equal(
    (await call({ kind: 'click', element: starId, framed: true })).ok,
    true,
    'private native click retains the visible hidden-radio proxy',
  );
  assert.equal((await call({ kind: 'eval', expression: "document.getElementById('s1').checked" })).data.value, true);
  assert.equal(
    (await call({ kind: 'eval', expression: 'typeof window.__acDiagnosticSemantics' })).data.value,
    'undefined',
    'semantic hook stays outside the page world',
  );
  await verifySemanticsOnly();
  console.log(
    'PASS Oya diagnostics: duplicate origins, native frame IDs, eval JSON, overlays, zero-effect shield/personal refusals, unoccluded trusted native input, frame typing, same-URL staleness, CSP, shadow input, bounded IPC, nested/detached frames, uncertain timeout, default control names/options/title, canonical portal widgets, inert semantic bootstrap and private click/type routing',
  );
}
/** A semantics-only initializer remains inert after body creation; later full analysis still works. */
async function verifySemanticsOnly() {
  const source = fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8');
  const world = 1009,
    execute = (code) => view.webContents.executeJavaScriptInIsolatedWorld(world, [{ code }]);
  await execute(`(()=>{const __OYA_ANALYZER_SEMANTICS_ONLY__=true;${source}\n})()`);
  assert.deepEqual(
    await execute(
      '({hook:!!window.__acDiagnosticSemantics,full:!!window.__acAnalyzerLoaded,analyze:typeof window.analyzePage})',
    ),
    { hook: true, full: false, analyze: 'undefined' },
  );
  await view.webContents.executeJavaScript("document.body.append(document.createElement('button'))");
  await pause(300);
  assert.equal(
    await view.webContents.executeJavaScript("document.querySelector('[__OYA_ATTR__]')===null"),
    true,
    'semantic initialization and mutations never tag canonical IDs',
  );
  await execute(source);
  assert.equal(
    await execute("window.__acAnalyzerLoaded===true&&typeof window.analyzePage==='function'"),
    true,
    'semantic hook cannot suppress full initialization',
  );
  await execute('window.analyzePage();window.__acCleanup();true');
}
/** Release only the window, views, server and profile owned by this test. */
function finish(code) {
  clearTimeout(deadline);
  for (const owned of [personal, shield?.view, view])
    if (owned?.webContents && !owned.webContents.isDestroyed()) owned.webContents.close();
  if (win && !win.isDestroyed()) win.destroy();
  server.closeAllConnections();
  server.close();
  app.exit(code);
}
app
  .whenReady()
  .then(run)
  .then(
    () => finish(0),
    (e) => {
      console.error(e);
      finish(1);
    },
  );
