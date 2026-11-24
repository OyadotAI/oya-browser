/** Real Oya DOM, isolated analyzer, native zoom and production shield; no debugger or third-party browser. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { setTimeout: pause } = require('node:timers/promises');
const electron = require('electron');
const { app, BrowserWindow, BrowserView } = electron;
const { World } = require('../../src/main/native/index.ts');
const { highlightMeasureJs } = require('../../src/main/shell/highlight-measure.ts');
const { HighlightReader } = require('../../src/main/shell/highlight-reader.ts');
const { ControlShield } = require('../../src/main/shell/control-shield.ts');
const profile = process.env.OYA_HIGHLIGHTS_PROFILE;
if (!profile) throw Error('Run desktop-highlights.mjs');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
app.on('web-contents-created', (_event, contents) => {
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw Error('Highlights accessed a debugger');
    },
  });
});
const deadline = setTimeout(() => {
  console.error('Highlight test timed out');
  app.exit(1);
}, 45000);
const world = new World({
  analyzerScript: fs.readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'highlight-fixture',
});
const markup = `<!doctype html><style>body{margin:0;height:2000px;background:#f8faf7;font:16px system-ui}button,input{box-sizing:border-box}
#normal{position:absolute;left:80px;top:100px;width:120px;height:40px}
#clip{position:absolute;left:80px;top:220px;width:200px;height:60px;overflow:hidden}
#partial{position:absolute;left:20px;top:40px;width:100px;height:60px}
iframe{position:absolute;left:400px;top:100px;width:300px;height:200px;border:10px solid #bbb;transform:scale(.75);transform-origin:0 0}
#host{position:absolute;left:400px;top:350px}#cover{display:none;position:absolute;left:70px;top:90px;width:150px;height:70px;background:#eee;z-index:10}</style>
<div style="position:absolute;left:80px;top:400px"><input id="custom-radio" type="radio" name="custom" style="position:absolute;opacity:0;width:20px;height:30px;margin:0"><label for="custom-radio" style="display:block;width:160px;height:30px">Styled radio</label></div>
<div style="position:absolute;left:80px;top:450px"><input id="custom-check" type="checkbox" style="opacity:0;position:absolute;width:20px;height:30px;margin:0"><label for="custom-check" style="display:block;width:160px;height:30px">Styled check</label></div>
<div id="toolbar" style="position:absolute;left:400px;top:450px;width:280px;height:45px;background:#eee"><ui-button id="disabled-reply" role="button" aria-label="Reply" aria-disabled="true" style="display:block;width:100px;height:40px;opacity:.4;pointer-events:none">Reply</ui-button><button id="disabled-trash" disabled style="position:absolute;left:150px;top:0;width:100px;height:40px;pointer-events:none">Trash</button></div>
<div id="toolbar-cover" style="display:none;position:absolute;left:400px;top:450px;width:280px;height:45px;background:white;z-index:20"></div>
<h1 style="margin:30px 80px;font-size:24px">Oya highlight alignment</h1>
<button id="normal">Normal</button><div id="clip"><button id="partial">Clipped</button></div>
<iframe id="frame" srcdoc="<body style='margin:0'><button id='inner' style='position:absolute;left:30px;top:40px;width:120px;height:40px'>Frame button</button></body>"></iframe><div id="host"></div><div id="cover"></div>
<script>document.getElementById('host').attachShadow({mode:'open'}).innerHTML='<button id="shadow" style="width:140px;height:40px">Shadow button</button>';</script>`;

/** Common form patterns where the scan and the outlines used to disagree with what a person sees. */
const AGREEMENT = `<!doctype html><style>body{margin:0;font:16px system-ui}.f{position:relative;width:240px;margin:20px}
.f label{position:absolute;inset:0;padding:10px;color:#777}.f input{width:100%;padding:10px;box-sizing:border-box}</style>
<div class="f"><input id="email"><label for="email">Email</label></div>
<label style="display:inline-flex;margin:20px"><input type="checkbox" id="agree" style="opacity:0;width:0;height:0;margin:0"><span style="width:20px;height:20px;border:1px solid"></span> Agree</label>
<div style="opacity:0;position:fixed;right:0;top:0;width:200px;height:60px"><button id="faded">Accept all</button></div>
<div style="position:fixed;left:0;top:300px;width:400px;height:80px;background:#ddd;z-index:5"></div><input id="under" style="position:absolute;left:20px;top:320px">`;
/** The scan and the outlines agree on floating labels, hidden toggles, faded banners and real overlays. */
async function scanMatchesOutlines(win) {
  const file = path.join(profile, 'agreement.html');
  fs.writeFileSync(file, AGREEMENT);
  const page = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true } });
  win.addBrowserView(page);
  page.setBounds({ x: 0, y: 0, width: 1080, height: 800 });
  await page.webContents.loadFile(file);
  const elements = (await world.evaluate(page, 'analyzePage({highlight:false})')).data.elements;
  const by = (text) => elements.find((e) => e.text === text);
  const outlined = new Set(
    (await world.evaluate(page, highlightMeasureJs(elements, 'capture', 'agree'))).map((b) => b.id),
  );
  const email = by('Email');
  assert.ok(email?.visible && !email.covered, 'A floating label over its own field does not cover it');
  assert.ok(outlined.has(email.id), 'A field under its own floating label is outlined');
  const agree = by('Agree');
  assert.ok(
    agree?.visible && outlined.has(agree.id),
    'A hidden checkbox is seen and outlined through its visible label',
  );
  assert.equal(by('Accept all')?.visible, false, 'A control inside a fully transparent banner is not visible');
  const under = by('');
  assert.ok(under?.covered && !outlined.has(under.id), 'A real overlay still covers the field beneath it');
  win.removeBrowserView(page);
}

/** Bounded polling for native layout and renderer readiness. */
async function until(read, message) {
  for (let n = 0; n < 150; n++) {
    if (await read()) return;
    await pause(20);
  }
  throw Error(message);
}
/** Match measured bounds against independent, fixed fixture geometry. */
function boxAt(boxes, id, expected) {
  const box = boxes.find((b) => b.id === id);
  assert.ok(box, `Missing highlight ${id}`);
  for (const [key, value] of Object.entries(expected))
    assert.ok(Math.abs(box[key] - value) < 1, `${id}.${key}: ${box[key]} != ${value}`);
}
/** Use the public shield surface and its real rendered outlines after initial measurement. */
async function rendered(win, page, elements) {
  const deps = {
    electron,
    appDir: path.resolve(__dirname, '../..'),
    world,
    control: { snapshot: () => ({ interactive: false }) },
    agentTab: { humanMay: () => false },
    shell: { window: win, browsingMode: true, send() {}, alive: () => true },
    tabs: { getShownView: () => page, getActiveView: () => page },
    shortcuts: { install() {} },
    overlays: { names: new Set() },
    recorder: { controlLost() {} },
  };
  const shield = new ControlShield(deps);
  shield.sync();
  await until(() => shield.view.webContents.executeJavaScript('!!window.oyaShield').catch(() => false), 'Shield ready');
  await shield.analysisFinished(page, { data: { elements } });
  await until(
    () => shield.view.webContents.executeJavaScript('document.querySelectorAll(".box").length>=3'),
    'Rendered outlines',
  );
  await until(
    () => shield.view.webContents.executeJavaScript('!!document.querySelector(".box.disabled")'),
    'Disabled toolbar controls remain outlined',
  );
  assert.equal(
    await shield.view.webContents.executeJavaScript(
      'getComputedStyle(document.querySelector(".box.disabled"), "::before").borderTopStyle',
    ),
    'dashed',
  );
  await page.webContents.executeJavaScript('document.getElementById("normal").style.top="180px"');
  await until(
    () =>
      shield.view.webContents.executeJavaScript(
        `document.querySelector('.box[data-id="${elements.find((e) => e.text === 'Normal').id}"]')?.style.transform.includes('180px')`,
      ),
    'Outline follows layout',
  );
  assert.equal(
    await shield.view.webContents.executeJavaScript(
      'getComputedStyle(document.querySelector(".box")).transitionProperty',
    ),
    'opacity',
  );
  await page.webContents.executeJavaScript(
    'document.getElementById("cover").style.top="170px";document.getElementById("cover").style.display="block"',
  );
  await until(
    () =>
      shield.view.webContents.executeJavaScript(
        `document.querySelector('.box[data-id="${elements.find((e) => e.text === 'Normal').id}"]')?.classList.contains('gone')`,
      ),
    'Covered outline disappears',
  );
  await page.webContents.executeJavaScript('document.getElementById("cover").style.display="none"');
  await pause(100);
  fs.writeFileSync('/private/tmp/oya-highlight-overlay.png', (await shield.view.webContents.capturePage()).toPNG());
  fs.writeFileSync('/private/tmp/oya-highlight-page.png', (await page.webContents.capturePage()).toPNG());
  await page.webContents.loadURL('about:blank');
  await until(
    () => shield.view.webContents.executeJavaScript('!document.querySelector(".box,.target")'),
    'Navigation clears geometry',
  );
  shield.stopTracking();
}
/** Disabled pointer-transparent controls remain visible, but never shine through a real covering element. */
async function disabledControls(page, measure, boxes, ids) {
  boxAt(boxes, ids.Reply, { x: 400, y: 450, w: 100, h: 40 });
  boxAt(boxes, ids.Trash, { x: 550, y: 450, w: 100, h: 40 });
  assert.ok(boxes.find((b) => b.id === ids.Reply).disabled);
  assert.ok(boxes.find((b) => b.id === ids.Trash).disabled);
  await page.webContents.executeJavaScript('document.getElementById("toolbar-cover").style.display="block"');
  assert.ok(
    !(await measure('follow')).some((b) => [ids.Reply, ids.Trash].includes(b.id)),
    'An unrelated overlay still hides disabled controls',
  );
  await page.webContents.executeJavaScript(
    'document.getElementById("toolbar-cover").style.display="none";document.getElementById("disabled-reply").setAttribute("aria-disabled","false")',
  );
  assert.ok(
    !(await measure('follow')).some((b) => b.id === ids.Reply),
    'Enabled pointer-transparent controls do not borrow ancestor hit tests',
  );
  await page.webContents.executeJavaScript('document.getElementById("disabled-reply").style.pointerEvents="auto"');
  assert.ok(
    !(await measure('follow')).find((b) => b.id === ids.Reply).disabled,
    'Availability updates while following the same node',
  );
  await page.webContents.executeJavaScript(
    'document.getElementById("disabled-reply").setAttribute("aria-disabled","true");document.getElementById("disabled-reply").style.pointerEvents="none"',
  );
}
/** Exercise clipped containers, scaled and shadow frames, identity replacement, zoom, and live overlay placement. */
async function run() {
  await app.whenReady();
  const file = path.join(profile, 'fixture.html');
  fs.writeFileSync(file, markup);
  const win = new BrowserWindow({ width: 1080, height: 840, show: true, webPreferences: { sandbox: true } });
  await win.loadURL('about:blank');
  const page = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true } });
  win.addBrowserView(page);
  page.setBounds({ x: 0, y: 0, width: 1080, height: 800 });
  await page.webContents.loadFile(file);
  const analysis = await world.evaluate(page, 'analyzePage({highlight:false})');
  const elements = analysis.data.elements;
  const ids = Object.fromEntries(elements.map((e) => [e.text || e.label, e.id]));
  const normal = elements.find((e) => e.text === 'Normal');
  assert.ok(normal);
  assert.ok(ids['Frame button']);
  const measure = (mode, key = 'scan') => world.evaluate(page, highlightMeasureJs(elements, mode, key));
  const beforeMeasure = await page.webContents.executeJavaScript('document.documentElement.outerHTML');
  const boxes = await measure('capture');
  assert.equal(
    await page.webContents.executeJavaScript('document.documentElement.outerHTML'),
    beforeMeasure,
    'Measuring must not add page DOM or styles',
  );
  assert.deepEqual(
    await page.webContents.executeJavaScript(
      '[typeof analyzePage, typeof __oyaHighlightMeasure, document.querySelector("[data-ac-id]") !== null]',
    ),
    ['undefined', 'undefined', false],
    'Analyzer and measurement state stay in the isolated world without a fixed public marker',
  );
  await disabledControls(page, measure, boxes, ids);
  boxAt(boxes, normal.id, { x: 80, y: 100, w: 120, h: 40 });
  boxAt(boxes, ids.Clipped, { x: 100, y: 260, w: 100, h: 20 });
  boxAt(boxes, ids['Frame button'], { x: 430, y: 137.5, w: 90, h: 30 });
  boxAt(boxes, ids['Shadow button'], { x: 400, y: 350, w: 140, h: 40 });
  boxAt(boxes, ids['Styled radio'], { x: 80, y: 400, w: 160, h: 30 });
  boxAt(boxes, ids['Styled check'], { x: 80, y: 450, w: 160, h: 30 });
  await page.webContents.executeJavaScript('document.querySelector("label[for=custom-radio]").htmlFor="unrelated"');
  assert.ok(
    !(await measure('follow')).some((b) => b.id === ids['Styled radio']),
    'Detached label association cannot highlight another control',
  );
  await page.webContents.executeJavaScript(
    'document.querySelector("label[for=custom-check]").style.visibility="hidden"',
  );
  assert.ok(
    !(await measure('follow')).some((b) => b.id === ids['Styled check']),
    'Hidden labels are not highlight proxies',
  );
  await page.webContents.executeJavaScript('document.getElementById("frame").style.transform="rotate(12deg)"');
  assert.ok(
    !(await measure('follow')).some((b) => b.id === ids['Frame button']),
    'Rotated frame must not get an inaccurate rectangle',
  );
  await page.webContents.executeJavaScript(
    'document.getElementById("frame").style.transform="scale(.75)"; document.getElementById("normal").outerHTML=document.getElementById("normal").outerHTML',
  );
  assert.ok(
    !(await measure('follow')).some((b) => b.id === normal.id),
    'Reused attributes must not retarget the old highlight',
  );
  assert.deepEqual(await measure('follow', 'obsolete'), [], 'An obsolete scan cannot borrow another scan’s nodes');
  page.webContents.setZoomFactor(1.25);
  await pause(80);
  const reader = new HighlightReader({ world }, () => ({
    webContents: { isDestroyed: () => false, getZoomFactor: () => 1 },
  }));
  const zoomed = await reader.measure(page, highlightMeasureJs(elements, 'capture', 'zoomed'));
  boxAt(zoomed, normal.id, { x: 100, y: 125, w: 150, h: 50 });
  page.webContents.setZoomFactor(1);
  await pause(80);
  const refreshed = (await world.evaluate(page, 'analyzePage({highlight:false})')).data.elements;
  await rendered(win, page, refreshed);
  await scanMatchesOutlines(win);
  console.log(
    'PASS Oya highlights: clipping, scaled iframe, shadow DOM, exact-node tracking, rotation refusal, live movement, occlusion and navigation reset',
  );
  win.destroy();
}
run()
  .then(() => {
    clearTimeout(deadline);
    app.exit(0);
  })
  .catch((e) => {
    console.error(e);
    app.exit(1);
  });
