// Real Electron regression for the dashboard's cloud/desktop browser transport.
// npm run test:recording --prefix browser (Linux CI uses xvfb-run).
const { app, BrowserWindow, BrowserView } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const vm = require('node:vm');
const { createServer } = require('node:http');
const { RecordingChannel } = require('./scripts/recording.cjs');
const { LoginState } = require('./login-state');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-recorder-electron-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('disable-gpu');
app.once('quit', () => fs.rmSync(profile, { recursive: true, force: true }));
const analyzerScript = fs.readFileSync(path.join(__dirname, 'scripts/analyzer.js'), 'utf8');
const main = fs.readFileSync(path.join(__dirname, 'main.js'), 'utf8');
// Use the app's actual analyzer context creation/recreation alongside recording.
const runtime = main.slice(main.indexOf('async function ensureWorld('), main.indexOf('const KEY_DEFS ='));
const ISOLATED_WORLD = 'test-recording-world';
let server, win, channel;
(async () => {
  await app.whenReady();
  setTimeout(() => { console.error('Electron recording test timed out'); app.exit(1); }, 20000).unref();
  server = createServer((req, res) => {
    if (req.url === '/redirect') { res.writeHead(302, { Location: `http://localhost:${server.address().port}/login` }); return res.end(); }
    res.setHeader('Content-Type', 'text/html');
    res.end(req.url.startsWith('/feed')
      ? '<!doctype html><title>Feed</title><input id="message" placeholder="Message">'
      : `<!doctype html><title>Login fixture</title><form action="/feed" method="post">
        <label>Username<input id="username" name="username"></label>
        <label>Password<input id="password" name="password" type="password"></label>
        <button id="submit">Sign in</button></form>`);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  // A visible surface is necessary for real coordinate clicks, just as the
  // cloud app runs under Xvfb. No user's browser profile or accounts are used.
  win = new BrowserWindow({ show: true, width: 920, height: 740 });
  const view = new BrowserView({ webPreferences: { contextIsolation: true, sandbox: true, partition: 'recorder-test' } });
  win.setBrowserView(view); view.setBounds({ x: 0, y: 0, width: 900, height: 700 });
  await view.webContents.loadURL('about:blank');
  const dbg = view.webContents.debugger; dbg.attach('1.3');
  const sent = [];
  const send = (method, params) => { sent.push(method); return dbg.sendCommand(method, params); };
  const on = (method, fn) => {
    const listener = (_e, event, params) => { if (method === event) fn(params); };
    dbg.on('message', listener); return () => dbg.off('message', listener);
  };
  // Do not enable Runtime here: the production Electron setup doesn't.
  await send('Page.enable');
  await new LoginState().attach(send, on);
  const context = { require, ISOLATED_WORLD, worldContexts: new WeakMap(), analyzerScript,
    cdp: (_view, method, params) => send(method, params) };
  vm.createContext(context); vm.runInContext(runtime, context);
  view.webContents.on('did-finish-load', () => context.ensureWorld(view, { force: true }).catch(console.error));
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/start`);
  const steps = [], secrets = new Set();
  const start = async () => {
    channel = new RecordingChannel({ send, on, worldName: ISOLATED_WORLD, disableRuntimeOnStop: true,
      analyzer: analyzerScript.replace('__OYA_ATTR__', 'data-test-recorder').replace('__OYA_RECORD__', 'false'),
      receive: out => { steps.push(...(out.steps || [])); for (const name of out.secrets || []) secrets.add(name); },
    });
    await channel.start();
  };
  const click = async (id) => {
    const point = await view.webContents.executeJavaScript(`(() => { const r = document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
  };
  await start();
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/redirect`);
  // Polling creates/uses the ordinary analyzer context; it must not switch the
  // recorder to that different, same-document world.
  await context.worldEval(view, 'analyzePage()');
  await channel.drain();
  await click('username'); await send('Input.insertText', { text: 'fixture-user' });
  await click('password'); await send('Input.insertText', { text: 'fixture-secret' });
  const loaded = new Promise(resolve => view.webContents.once('did-finish-load', resolve));
  await click('submit'); await loaded;
  await send('Page.captureScreenshot', { format: 'png' });
  // Stop while the next page's field is still focused (no change/blur event).
  await click('message'); await send('Input.insertText', { text: 'final-field-value' });
  await channel.stop(); channel = null;
  assert(steps.some(s => s.action === 'type' && s.text === 'fixture-user'), 'username captured after cross-site navigation');
  assert(steps.some(s => s.action === 'type' && s.text === '{{password}}'), 'password captured as placeholder');
  assert(steps.some(s => s.action === 'click' && s.el.domId === 'submit'), 'submit captured before navigation');
  assert(steps.some(s => s.action === 'type' && s.text === 'final-field-value'), 'stop flushes the recorder world after navigation');
  assert(!JSON.stringify(steps).includes('fixture-secret'), 'raw password never exported');
  assert(secrets.has('password'));
  assert.equal(sent.at(-1), 'Runtime.disable', 'desktop event reporting ends with recording');
  const count = steps.length;
  await send('Input.insertText', { text: 'not-recorded' });
  await click('message');
  assert.equal(steps.length, count, 'stop disarms the page listeners');
  await start();
  await click('message'); await send('Input.insertText', { text: 'discard-me' });
  await channel.clear();
  steps.length = 0; secrets.clear();
  await channel.stop(); channel = null;
  assert.equal(steps.length, 0, 'clear drops an unflushed field in the actual recorder world');
  await view.webContents.executeJavaScript(`document.body.innerHTML = \`
    <input id="visible"><input id="hidden" type="hidden">
    <div style="display:none"><input id="ancestorHidden"></div>
    <input id="transparent" style="opacity:0">
    <input id="invisible" style="visibility:hidden">
    <button id="custom" onclick="hidden.value='background'; hidden.dispatchEvent(new Event('input', {bubbles:true}))">Choose</button>
    <label id="checkLabel" for="check">Agree</label><input id="check" type="checkbox" style="display:none">
    <label id="nativeLabel" for="nativeCheck">Native</label><input id="nativeCheck" type="checkbox">
    <select id="choice" size="2"><option selected>One</option><option>Two</option></select>
    <input id="vanishing"><button id="keyboardButton">Submit</button>\``);
  await start();
  await view.webContents.executeJavaScript(`(() => {
    for (const id of ['visible', 'hidden', 'ancestorHidden', 'transparent', 'invisible']) {
      const el = document.getElementById(id); el.value = 'background';
      for (const type of ['focusin', 'input', 'change', 'click']) el.dispatchEvent(new Event(type, {bubbles:true}));
      el.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', bubbles:true}));
    }
    document.getElementById('custom').click();
  })()`);
  await channel.drain(true);
  assert.equal(steps.length, 0, 'script-dispatched events never record, even on visible fields');
  // Browser-generated focus events are trusted, even when script focuses an invisible field.
  for (const id of ['transparent', 'invisible', 'ancestorHidden', 'hidden']) {
    await view.webContents.executeJavaScript(`document.getElementById(${JSON.stringify(id)}).focus()`);
    await send('Input.insertText', { text: 'ignored' });
  }
  await channel.drain(true);
  assert.equal(steps.length, 0, 'hidden targets do not produce edits');
  await click('custom');
  await click('checkLabel');
  await click('nativeLabel');
  await view.webContents.executeJavaScript(`document.getElementById('choice').focus()`);
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 });
  await click('visible');
  await view.webContents.executeJavaScript(`document.getElementById('visible').select()`);
  await send('Input.insertText', { text: 'real edit' });
  await click('vanishing'); await send('Input.insertText', { text: 'keep this edit' });
  await view.webContents.executeJavaScript(`document.getElementById('vanishing').remove(); document.getElementById('keyboardButton').focus()`);
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  await channel.stop(); channel = null;
  assert.equal(steps.filter(s => s.el?.domId === 'custom').length, 1, 'custom control records only the visible interaction');
  assert.equal(steps.filter(s => s.el?.domId === 'checkLabel').length, 1, 'hidden checkbox records its visible label');
  assert.equal(steps.filter(s => s.el?.domId === 'nativeCheck').length, 1, 'native label activation records one checkbox click');
  assert(!steps.some(s => ['hidden', 'ancestorHidden', 'transparent', 'invisible', 'check'].includes(s.el?.domId)));
  assert(steps.some(s => s.action === 'select_option' && s.option === 'Two'), 'native keyboard selection captured');
  assert.deepEqual(steps.filter(s => s.action === 'type').map(s => s.text), ['real edit', 'keep this edit'], 'real edits survive field removal in order');
  assert.equal(steps.filter(s => s.action === 'press_key' && s.key === 'Enter').length, 1);
  assert(!steps.some(s => s.action === 'click' && s.el?.domId === 'keyboardButton'), 'Enter activation is not duplicated');
  console.log('Electron recording: navigation, secrets, hidden/synthetic events, custom controls, labels, selection, field removal, keyboard submission, stop and clear passed');
})().catch(err => { console.error(err); process.exitCode = 1; }).finally(async () => {
  await channel?.stop().catch(() => {});
  win?.destroy(); server?.close();
  app.exit(process.exitCode || 0);
});
