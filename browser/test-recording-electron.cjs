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
  console.log('Electron recording: cross-site login, secret masking, submit, final field, stop and clear passed');
})().catch(err => { console.error(err); process.exitCode = 1; }).finally(async () => {
  await channel?.stop().catch(() => {});
  win?.destroy(); server?.close();
  app.exit(process.exitCode || 0);
});
