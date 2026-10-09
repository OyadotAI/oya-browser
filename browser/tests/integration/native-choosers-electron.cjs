/** Hermetic native popup and file-chooser regressions; no CDP and no external network. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { World } = require('../../src/main/native/index.ts');
const { findElementJs } = require('../../src/main/actions/scripts.ts');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-native-choosers-'));
app.setPath('userData', path.join(dir, 'profile'));
const keyboard = new Keyboard(process.platform),
  mouse = new Mouse();
const world = new World({
  analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'native-choosers',
});
const html = path.join(dir, 'fixture.html');
const file = path.join(dir, 'generated.txt');
fs.writeFileSync(file, 'Generated native chooser regression.');
fs.writeFileSync(
  html,
  `<!doctype html><select><option value="" disabled selected>Choose</option><option value="1">One</option><option value="2">Two</option></select>
  <input type="file"><output></output><script>document.addEventListener('change',e=>document.querySelector('output').textContent=e.isTrusted?'trusted':'synthetic')</script>`,
);
const deadline = setTimeout(() => {
  console.error('Native chooser deadline exceeded');
  app.exit(1);
}, 60000);
/** Bound observations even if a renderer reply stalls during a native popup. */
async function until(fn) {
  let timer;
  const loop = async () => {
    for (let i = 0; i < 100; i++) {
      if (await fn()) return;
      await new Promise((r) => setTimeout(r, 25));
    }
    throw Error('Chooser condition timed out');
  };
  try {
    await Promise.race([
      loop(),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error('Native chooser observation stalled')), 5000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
/** Resolve analyzer ids and click through the existing native pointer path. */
async function click(view, tag) {
  const a = await world.evaluate(view, 'analyzePage({})');
  const el = a.data.elements.find((e) => e.tag === tag);
  assert.ok(el);
  const point = await world.evaluate(view, findElementJs(String(el.id)));
  assert.equal(point.ok, true);
  await mouse.click(view, point.data.x, point.data.y);
}
/** Exercise real AppKit menu input, trusted file changes, cancellation and navigation isolation. */
async function run() {
  const win = new BrowserWindow({
    width: 800,
    height: 600,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  const wc = win.webContents,
    view = { webContents: wc };
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  await win.loadFile(html);
  win.show();
  win.focus();
  wc.focus();
  await until(() => win.isFocused());
  await click(view, 'select');
  await new Promise((r) => setTimeout(r, 150));
  await keyboard.press(view, 'End');
  await keyboard.press(view, 'Enter');
  await until(async () => (await world.evaluate(view, 'document.querySelector("select").value')) === '2');
  assert.equal(await world.evaluate(view, 'document.querySelector("output").textContent'), 'trusted');
  console.log('PASS: native select popup keyboard selection emits a trusted change');
  await click(view, 'select');
  await new Promise((r) => setTimeout(r, 150));
  await keyboard.press(view, 'Home');
  await keyboard.press(view, 'Escape');
  assert.equal(await world.evaluate(view, 'document.querySelector("select").value'), '2');
  console.log('PASS: native popup Escape cancels without changing the selection');
  let info, reply;
  const handler = (event, details, answer) => {
    event.preventDefault();
    info = details;
    reply = answer;
  };
  wc.on('-oya-file-chooser', handler);
  await click(view, 'input');
  await until(() => !!reply);
  assert.equal(info.processId, wc.mainFrame.processId);
  assert.equal(info.routingId, wc.mainFrame.routingId);
  reply([file]);
  await until(async () => (await world.evaluate(view, 'document.querySelector("input").files.length')) === 1);
  assert.equal(await world.evaluate(view, 'document.querySelector("input").files[0].name'), 'generated.txt');
  assert.equal(
    await world.evaluate(view, 'document.querySelector("input").files[0].text()'),
    fs.readFileSync(file, 'utf8'),
  );
  assert.equal(await world.evaluate(view, 'document.querySelector("output").textContent'), 'trusted');
  console.log('PASS: native file chooser selects the authorized generated file');
  reply = undefined;
  await click(view, 'input');
  await until(() => !!reply);
  reply([]);
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(await world.evaluate(view, 'document.querySelector("input").files[0].name'), 'generated.txt');
  reply = undefined;
  await click(view, 'input');
  await until(() => !!reply);
  const stale = reply;
  await win.loadFile(html);
  stale([file]);
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(await world.evaluate(view, 'document.querySelector("input").files.length'), 0);
  console.log('PASS: cancellation preserves selection and stale file replies cannot cross navigation');
  for (const paths of [['relative.txt'], [file, file]]) {
    reply = undefined;
    await click(view, 'input');
    await until(() => !!reply);
    reply(paths);
    await new Promise((r) => setTimeout(r, 100));
    assert.equal(await world.evaluate(view, 'document.querySelector("input").files.length'), 0);
  }
  console.log('PASS: relative paths and multiple files for a single-file chooser are refused');
  wc.off('-oya-file-chooser', handler);
  win.destroy();
}
app
  .whenReady()
  .then(run)
  .then(
    () => {
      clearTimeout(deadline);
      app.exit(0);
    },
    (error) => {
      console.error(error);
      clearTimeout(deadline);
      app.exit(1);
    },
  );
