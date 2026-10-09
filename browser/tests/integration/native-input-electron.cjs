/** Oya-native pointer regression: all debugger access is fatal, including implicit attachment. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const { World, capturePage, evaluatePage } = require('../../src/main/native/index.ts');
const { pickTarget } = require('../../src/main/workflow/target-picker.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { stillWhileAway } = require('../../src/main/shell/hold-still.ts');
const { pathToFileURL } = require('node:url');
const { POINTER_COMMANDS } = require('../../src/main/actions/pointer-commands.ts');
const profile = process.env.OYA_NATIVE_INPUT_PROFILE;
if (!profile) throw new Error('Run native-input.mjs so the parent owns profile cleanup after Electron exits');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
app.commandLine.appendSwitch('disable-backgrounding-occluded-windows');
app.commandLine.appendSwitch('disable-renderer-backgrounding');
const fixture = path.join(profile, 'fixture.html');
fs.writeFileSync(
  fixture,
  `<!doctype html><style>body{margin:0;height:3000px}button{position:absolute;left:20px;top:20px;width:120px;height:50px}#drag{position:absolute;top:100px;width:300px;height:100px;background:#ccc}</style><button>Native input</button><div id="drag"></div><script>window.events=[]; for(const name of ['click','dblclick','mousemove','mousedown','mouseup','wheel'])document.addEventListener(name,e=>events.push({type:e.type,trusted:e.isTrusted,buttons:e.buttons,x:e.clientX,y:e.clientY}));</script>`,
);
const deadline = setTimeout(() => {
  console.error('Native input test timed out');
  app.exit(1);
}, 60000);
/** Bounded polling observes native event delivery, not an arbitrary paint delay. */
async function until(read) {
  for (let i = 0; i < 100; i++) {
    if (await read()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error('Native input condition did not become true');
}
/** Selection input is intercepted natively, including at non-default page zoom. */
async function checkPicker(win) {
  const wc = win.webContents;
  const source = path.join(profile, 'picker.html');
  fs.writeFileSync(
    source,
    '<!doctype html><style>body{margin:0}button{position:absolute;left:20px;top:20px;width:120px;height:50px}</style><button id="save">Save</button><script>window.clicks=0;document.querySelector("button").onclick=()=>clicks++</script>',
  );
  await win.loadFile(source);
  for (const zoom of [1, 2]) {
    wc.setZoomFactor(zoom);
    const picked = pickTarget({ webContents: wc });
    picked.catch(() => {});
    await until(() => wc.executeJavaScript('!!document.querySelector("[data-oya-picker]")'));
    wc.sendInputEvent({ type: 'mouseMove', x: 80 * zoom, y: 45 * zoom });
    await until(() => wc.executeJavaScript('document.querySelector("[data-oya-picker]").style.display === "block"'));
    wc.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, x: 80 * zoom, y: 45 * zoom });
    wc.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, x: 80 * zoom, y: 45 * zoom });
    assert.ok((await picked).some((choice) => choice.kind === 'role' && choice.value === 'Save'));
    await until(() => !wc.oyaTargetPicking);
    assert.equal(await wc.executeJavaScript('clicks'), 0);
    assert.equal(await wc.executeJavaScript('typeof __oyaPicker'), 'undefined');
    assert.equal(await wc.executeJavaScript('!!document.querySelector("[data-oya-picker]")'), false);
  }
  wc.setZoomFactor(1);
  const cancelled = pickTarget({ webContents: wc });
  cancelled.catch(() => {});
  await until(() => wc.executeJavaScript('!!document.querySelector("[data-oya-picker]")'));
  wc.sendInputEvent({ type: 'keyDown', keyCode: 'Escape' });
  await assert.rejects(cancelled, /canceled/);
  await until(() => !wc.oyaTargetPicking);
  await win.loadFile(fixture);
}
/** Exercise actual CSS and first-paint policy while the system media query remains untouched. */
async function checkShellMotion(win) {
  const asset = (name) => pathToFileURL(path.resolve(__dirname, '../../src/renderer/public', name)).href;
  const motion = path.join(profile, 'motion.html');
  fs.writeFileSync(
    motion,
    `<!doctype html><script src="${asset('first-paint.js')}"></script><link rel="stylesheet" href="${asset('shell-motion.css')}"><style>@keyframes spin{to{transform:rotate(360deg)}}div{animation:spin 1s infinite}</style><div>Oya motion</div>`,
  );
  const wc = win.webContents;
  await win.loadFile(motion, { query: { still: 'true' } });
  const animation = () => wc.executeJavaScript('getComputedStyle(document.querySelector("div")).animationName');
  const preference = () => wc.executeJavaScript('matchMedia("(prefers-reduced-motion: reduce)").matches');
  assert.equal(await animation(), 'none');
  const systemMotion = await preference();
  stillWhileAway(win);
  win.emit('focus');
  await until(async () => (await animation()) === 'spin');
  win.emit('blur');
  await until(async () => (await animation()) === 'none');
  await win.loadFile(motion);
  assert.equal(await animation(), 'none');
  win.emit('focus');
  await until(async () => (await animation()) === 'spin');
  assert.equal(await preference(), systemMotion);
}
app
  .whenReady()
  .then(async () => {
    const win = new BrowserWindow({
      width: 800,
      height: 600,
      webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
    });
    const contents = win.webContents;
    Object.defineProperty(contents, 'debugger', {
      get() {
        throw new Error('Internal CDP is forbidden');
      },
    });
    await checkShellMotion(win);
    await win.loadFile(fixture);
    win.show();
    win.focus();
    contents.focus();
    await until(() => win.isFocused());
    await checkPicker(win);
    const view = { webContents: contents };
    assert.ok((await capturePage(view)).startsWith('data:image/png;base64,'));
    assert.ok((await capturePage(view, 80)).startsWith('data:image/jpeg;base64,'));
    assert.equal(await evaluatePage(view, '6*7'), 42);
    const world = new World({
      analyzerScript:
        'globalThis.nativeProbe = 41;' + fs.readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8'),
      worldName: 'oya-native-test',
    });
    assert.equal(await world.evaluate(view, 'nativeProbe + 1'), 42);
    assert.equal(await evaluatePage(view, 'typeof nativeProbe'), 'undefined');
    assert.equal(await evaluatePage(view, 'typeof analyzePage'), 'undefined');
    assert.equal(await world.evaluate(view, 'typeof analyzePage'), 'function');
    await contents.loadFile(fixture);
    assert.equal(await world.evaluate(view, 'nativeProbe + 1'), 42);
    contents.focus();
    const mouse = new Mouse();
    const keyboard = new Keyboard(process.platform);
    await contents.executeJavaScript(
      `{ const input = document.createElement('textarea'); input.id='native-text'; input.style='position:absolute;left:400px;top:20px'; document.body.append(input); input.focus(); }`,
    );
    await keyboard.type(view, 'aB/!é🙂');
    assert.equal(await contents.executeJavaScript("document.querySelector('#native-text').value"), 'aB/!é🙂');
    await keyboard.clear(view);
    await until(() => contents.executeJavaScript("document.querySelector('#native-text').value === ''"));
    await keyboard.type(view, 'line\nnext');
    await until(
      async () => (await contents.executeJavaScript("document.querySelector('#native-text').value")) === 'line\nnext',
    );
    await contents.executeJavaScript("document.querySelector('#native-text').remove()");
    await contents.executeJavaScript(
      `{ const form = document.createElement('form'); form.innerHTML='<input id="submit-field"><button>Submit</button>'; document.body.append(form); window.submits=0; form.onsubmit=e=>{e.preventDefault(); submits++;}; document.querySelector('#submit-field').focus(); }`,
    );
    await keyboard.press(view, 'Enter');
    await until(() => contents.executeJavaScript('submits === 1'));
    await contents.executeJavaScript(
      `document.querySelector('form').remove(); const frame = document.createElement('iframe'); frame.id='native-frame'; frame.srcdoc='<input id="inner">'; document.body.append(frame);`,
    );
    await until(() =>
      contents.executeJavaScript("!!document.querySelector('#native-frame').contentDocument?.querySelector('#inner')"),
    );
    await contents.executeJavaScript(
      "document.querySelector('#native-frame').contentDocument.querySelector('#inner').focus()",
    );
    await keyboard.type(view, 'frameé');
    assert.equal(
      await contents.executeJavaScript(
        "document.querySelector('#native-frame').contentDocument.querySelector('#inner').value",
      ),
      'frameé',
    );
    await contents.executeJavaScript("document.querySelector('#native-frame').remove()");

    await mouse.click(view, 70, 40);
    await until(() => contents.executeJavaScript('events.some(e=>e.type==="click" && e.trusted)'));
    const replies = [];
    const driver = { mouse, activeView: () => view, deps: { sendResult: (...args) => replies.push(args) } };
    await POINTER_COMMANDS.double_click(driver, 'double', { x: 70, y: 40 });
    await until(() => contents.executeJavaScript('events.some(e=>e.type==="dblclick" && e.trusted)'));
    await contents.executeJavaScript('events=[]');
    await POINTER_COMMANDS.drag(driver, 'drag', { from_x: 30, from_y: 130, to_x: 250, to_y: 150 });
    await until(() => contents.executeJavaScript('events.some(e=>e.type==="mouseup" && e.x===250)'));
    assert.equal(
      await contents.executeJavaScript('events.some(e=>e.type==="mousemove" && e.buttons===1 && e.trusted)'),
      true,
    );
    await mouse.scroll(view, 400, 300, 0, 200);
    await until(() => contents.executeJavaScript('scrollY >= 190'));
    const before = await contents.executeJavaScript('scrollY');
    await mouse.scroll(view, 400, 300, 0, -100);
    await until(() => contents.executeJavaScript(`scrollY < ${before}`));
    assert.ok(replies.every((reply) => reply[1] === true));
    await require('./native-page-commands.cjs')(win, profile);
    await require('./native-shielded-input.cjs')(win, profile);
    win.destroy();
    await assert.rejects(mouse.click(view, 1, 2), /destroyed/);
    console.log(
      'PASS: native Unicode typing, editing, Enter submission, frame focus, isolated analyzer, navigation, capture, trusted pointer and scroll; debugger access forbidden',
    );
  })
  .then(
    () => finish(0),
    (error) => {
      console.error(error);
      finish(1);
    },
  );
/** This test owns its disposable profile and no human browser session. */
function finish(code) {
  clearTimeout(deadline);
  app.exit(code);
}
