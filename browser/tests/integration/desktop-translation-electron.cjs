/** Native Oya translation, overlay stacking and keyboard checks; fake model, no network or debugger. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { setTimeout: pause } = require('node:timers/promises');
const electron = require('electron');
const { app, BrowserWindow, BrowserView, ipcMain } = electron;
const { Translator } = require('../../src/main/translate/index.ts');
const { Overlays } = require('../../src/main/shell/overlays.ts');
const profile = process.env.OYA_TRANSLATION_PROFILE;
if (!profile) throw Error('Run desktop-translation.mjs');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
app.on('web-contents-created', (_event, contents) => {
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw Error('Translation accessed a debugger');
    },
  });
});
const deadline = setTimeout(() => {
  console.error('Translation fixture timed out');
  app.exit(1);
}, 60000);
const markup =
  '<!doctype html><html lang="fr"><style>body{font:24px system-ui;padding:40px;color:#243d2b;background:#fafcf8}</style><h1>Bonjour</h1><p>Bienvenue dans notre jardin.</p></html>';

/** Bounded native readiness, without a browser automation library. */
async function until(read, message) {
  for (let n = 0; n < 200; n++) {
    if (await read()) return;
    await pause(20);
  }
  throw Error(message);
}
/** Dispatch a native key pair to the focused shell. */
function key(win, keyCode, modifiers = []) {
  win.webContents.sendInputEvent({ type: 'keyDown', keyCode, modifiers });
  if (keyCode === 'Enter') win.webContents.sendInputEvent({ type: 'char', keyCode: '\r', modifiers });
  win.webContents.sendInputEvent({ type: 'keyUp', keyCode, modifiers });
}
/** Click a real renderer control, including its normal focus behavior. */
async function click(win, selector) {
  win.focus();
  win.webContents.focus();
  const point = await win.webContents.executeJavaScript(
    `(() => { const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2) }; })()`,
  );
  win.webContents.sendInputEvent({ type: 'mouseDown', ...point, button: 'left', clickCount: 1 });
  win.webContents.sendInputEvent({ type: 'mouseUp', ...point, button: 'left', clickCount: 1 });
}

/** Compose production main services with a real page and production renderer components. */
async function fixture() {
  const win = new BrowserWindow({
    width: 1280,
    height: 860,
    useContentSize: true,
    webPreferences: { preload: path.join(__dirname, 'desktop-translation-preload.cjs'), contextIsolation: true },
  });
  const page = new BrowserView({ webPreferences: { contextIsolation: true, sandbox: true } });
  win.webContents.on('console-message', (_event, details) => console.log('fixture renderer:', details));
  const tab = { id: 1, url: 'https://documentation-des-jardins.example.fr/bonjour', view: page };
  const model = { calls: 0, pending: null, human: true, target: 'en-US' };
  const bounds = () => {
    const [width, height] = win.getContentSize();
    page.setBounds({ x: 0, y: 88, width, height: height - 88 });
  };
  const shell = { window: win, browsingMode: true, send: (name, payload) => win.webContents.send(name, payload) };
  const deps = {
    config: { values: {} },
    socket: {},
    governance: { configuration: null },
    recorder: { recording: false },
    agentTab: { humanMay: () => model.human },
    tabs: { list: [tab], getShownView: () => page },
    electron: { app: { getPreferredSystemLanguages: () => [model.target], getLocale: () => model.target } },
  };
  const overlays = new Overlays({ tabs: deps.tabs, shell, shield: { sync() {} }, layout: { layoutActiveTab: bounds } });
  const publish = () =>
    win.webContents.send('translation-tabs', [
      { id: tab.id, url: tab.url, active: true, translation: tab.translation },
    ]);
  const api = {
    canCall: () => true,
    postToBrowser: async (_route, body) => {
      model.calls++;
      if (model.pending) await model.pending;
      return Response.json({ texts: body.texts.map((text) => `English: ${text}`) });
    },
  };
  const translator = new Translator(deps, publish, api);
  const calls = {
    translatePage: () => translator.translate(tab),
    confirmTranslation: (_id, proposal) => translator.confirm(tab, proposal),
    showOriginal: () => translator.showOriginal(tab),
    showOverlay: (name) => overlays.show(name),
    hideOverlay: (name) => overlays.hide(name),
    backdropReady: (token) => overlays.backdropReady(token),
  };
  ipcMain.handle('translation-fixture', (event, name, args) => {
    assert.equal(event.sender, win.webContents);
    assert.ok(Object.hasOwn(calls, name));
    return calls[name](...args);
  });
  win.on('resize', bounds);
  await win.loadFile(path.join(profile, 'index.html'));
  await until(() => win.webContents.executeJavaScript('!!window.translationReady'), 'renderer ready');
  await page.webContents.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(markup)}`);
  win.addBrowserView(page);
  bounds();
  await translator.detect(tab);
  return { win, page, tab, model, deps, overlays, translator, publish };
}

/** Consent geometry and keyboard affordances remain usable at both supported sizes and themes. */
async function dialogChecks(f) {
  const { win, page, tab, model, translator, overlays } = f;
  const read = (code) => win.webContents.executeJavaScript(code);
  const shots = process.env.OYA_TRANSLATION_SCREENSHOTS;
  if (shots) fs.mkdirSync(shots, { recursive: true });
  for (const [width, height] of [
    [600, 400],
    [1280, 860],
  ])
    for (const theme of ['light', 'dark']) {
      win.setContentSize(width, height);
      win.focus();
      win.webContents.focus();
      await pause(150);
      await read(`document.documentElement.dataset.theme=${JSON.stringify(theme)}`);
      await until(() => read('!!document.getElementById("btn-translate")'), 'translate chip');
      await read('document.getElementById("btn-translate").focus()');
      key(win, 'Enter');
      await until(() => read('document.activeElement?.id === "translate-cancel"'), 'Cancel receives initial focus');
      assert.equal(model.calls, 0, 'No page text before consent');
      assert.equal(win.getBrowserViews().includes(page), false, 'Native page must be behind consent');
      await pause(650);
      const geometry = await read(
        '(() => {const d=document.querySelector(".translation-dialog"),r=d.getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:innerWidth,height:innerHeight,overflow:d.scrollWidth>d.clientWidth,buttons:[...d.querySelectorAll("button")].map(b=>b.getBoundingClientRect().width)}})()',
      );
      assert.ok(
        geometry.x >= 0 && geometry.y >= 0 && geometry.right <= geometry.width && geometry.bottom <= geometry.height,
      );
      assert.equal(geometry.overflow, false);
      assert.ok(geometry.buttons.every((width) => width > 50));
      if (shots)
        fs.writeFileSync(
          path.join(shots, `consent-${width}-${theme}.png`),
          (await win.webContents.capturePage()).toPNG(),
        );
      key(win, 'Tab', ['shift']);
      await until(() => read('document.activeElement?.id === "translate-confirm"'), 'Shift Tab stays in dialog');
      key(win, 'Tab');
      await until(() => read('document.activeElement?.id === "translate-cancel"'), 'Tab wraps to Cancel');
      await overlays.show('legacy');
      key(win, 'Escape');
      await until(() => read('document.getElementById("translate-overlay").hidden'), 'Escape dismisses consent');
      assert.equal(win.getBrowserViews().includes(page), false, 'Another overlay retains native ownership');
      overlays.hide('legacy');
      assert.equal(win.getBrowserViews().includes(page), true);
      await until(() => read('document.activeElement?.id === "btn-translate"'), 'focus returns to chip');
      assert.equal(tab.translation.state, 'offer');
    }
  await read('document.activeElement?.blur()');
  await translator.translate(tab);
  await until(() => read('document.activeElement?.id === "translate-cancel"'), 'native menu opens same consent');
  key(win, 'Escape');
  await until(() => read('document.activeElement?.id === "url-bar"'), 'native menu recovers address focus');
}

/** Real isolated-world writes are restored before takeover, and late model results cannot reapply. */
async function translationChecks(f) {
  const { win, page, tab, model, translator } = f;
  const body = () => page.webContents.executeJavaScript('document.body.innerText');
  await click(win, '#btn-translate');
  await until(
    () => win.webContents.executeJavaScript('document.activeElement?.id === "translate-cancel"'),
    'confirmation visible',
  );
  await click(win, '#translate-confirm');
  await until(() => Promise.resolve(tab.translation?.state === 'translated'), 'translation completes');
  assert.match(await body(), /English: Bonjour/);
  model.human = false;
  await translator.released(page);
  assert.doesNotMatch(await body(), /English:/);
  model.human = true;
  await translator.sync();
  const pending = Promise.withResolvers();
  model.pending = pending.promise;
  await translator.translate(tab);
  const running = translator.confirm(tab, tab.translation.confirmation.id);
  await until(() => Promise.resolve(model.calls === 2), 'pending batch started');
  await translator.showOriginal(tab);
  pending.resolve();
  await running;
  assert.doesNotMatch(await body(), /English:/);
  assert.equal(tab.translateKeep, undefined);
}

/** Long disclosures wrap, and compact state labels remain visible without relying on their icon. */
async function compactChecks(f) {
  const { win, tab, model, translator, publish } = f;
  win.setContentSize(600, 400);
  model.target = 'tzm';
  await translator.translate(tab);
  await until(
    () => win.webContents.executeJavaScript('document.activeElement?.id === "translate-cancel"'),
    'long target consent',
  );
  const copy = await win.webContents.executeJavaScript('document.querySelector(".translation-dialog").innerText');
  assert.match(copy, /Central Atlas Tamazight/);
  if (process.env.OYA_TRANSLATION_SCREENSHOTS)
    fs.writeFileSync(
      path.join(process.env.OYA_TRANSLATION_SCREENSHOTS, 'consent-long-language.png'),
      (await win.webContents.capturePage()).toPNG(),
    );
  await translator.showOriginal(tab);
  const cases = [
    ['offer', null, 'Translate'],
    ['translating', null, 'Stop'],
    ['translated', null, 'Original'],
    ['partial', null, 'Original'],
    ['failed', 'offline', 'Retry'],
    ['failed', 'signed_out', 'Sign in'],
    ['failed', 'llm_unconfigured', 'Set up'],
    ['failed', 'llm_quota', 'Limit reached'],
    ['failed', 'restore_failed', 'Reload'],
  ];
  for (const [state, code, label] of cases) {
    tab.translation = { state, lang: 'fr', target: 'en-US', ...(code ? { error: { code, message: code } } : {}) };
    publish();
    await until(
      () =>
        win.webContents.executeJavaScript(
          `document.querySelector('.translate-chip-short')?.innerText === ${JSON.stringify(label)}`,
        ),
      label,
    );
    assert.equal(
      await win.webContents.executeJavaScript(
        '(() => {const e=document.querySelector(".translate-chip-short"),r=e.getBoundingClientRect(); return r.width>0 && r.right<=innerWidth && e.scrollWidth<=e.clientWidth;})()',
      ),
      true,
      label,
    );
  }
  model.target = 'en-US';
  await translator.detect(tab);
}

/** Every assertion runs through Oya-owned native APIs; the model and site are fixture data. */
async function main() {
  await app.whenReady();
  const f = await fixture();
  await dialogChecks(f);
  await compactChecks(f);
  await translationChecks(f);
  console.log(
    'PASS Oya translation: four consent layouts, native stacking, focus/keyboard, explicit consent, restore and late-result fences',
  );
  f.page.webContents.close();
  f.win.destroy();
  clearTimeout(deadline);
  app.exit(0);
}
main().catch((error) => {
  console.error(error);
  clearTimeout(deadline);
  app.exit(1);
});
