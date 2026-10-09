/** Cross-origin native frame execution in Oya; debugger access is forbidden. */
const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { evaluateFrame, nativeFramePath, World } = require('../../src/main/native/index.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-native-frames-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Native frame test timed out');
  app.exit(1);
}, 30000);
const server = http.createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  if (req.url === '/never') return;
  if (req.url === '/loading') {
    res.write('<!doctype html><body><button id=ready>Ready now</button><script src=/never></script>');
    return;
  }
  const port = server.address().port;
  res.end(
    req.url === '/parent'
      ? `<!doctype html><iframe id="first" name="first" src="http://localhost:${port}/child"></iframe><iframe id="second" name="second" src="http://localhost:${port}/child"></iframe>`
      : '<!doctype html><input id="value" value="original"><script>window.pageSecret="page"</script>',
  );
});
/** Fixture origins differ by host, forcing a genuine cross-process child renderer. */
async function run() {
  await new Promise((resolve) => server.listen(0, resolve));
  const win = new BrowserWindow({
    show: false,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInSubFrames: true,
      preload: path.resolve(__dirname, '../../out/preload/recording.js'),
    },
  });
  const contents = win.webContents;
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw new Error('Internal CDP is forbidden');
    },
  });
  await win.loadURL(`http://127.0.0.1:${server.address().port}/parent`);
  const first = contents.mainFrame.frames[0];
  let second = contents.mainFrame.frames[1];
  assert.ok(first && second);
  assert.notEqual(first.processId, contents.mainFrame.processId);
  assert.equal(first.url, second.url);
  assert.equal(await evaluateFrame(first, 'typeof pageSecret'), 'undefined');
  assert.equal(
    await evaluateFrame(
      first,
      'globalThis.agentSecret=42; document.querySelector("input").value="changed"; Promise.resolve(agentSecret)',
    ),
    42,
  );
  assert.equal(await first.executeJavaScript('typeof agentSecret'), 'undefined');
  assert.equal(await first.executeJavaScript('document.querySelector("input").value'), 'changed');
  assert.equal(await evaluateFrame(second, 'typeof agentSecret'), 'undefined');
  assert.equal(await evaluateFrame(second, 'document.querySelector("input").value'), 'original');
  await assert.rejects(evaluateFrame(first, 'throw new Error("native-frame-failure")'), /native-frame-failure/);
  assert.deepEqual(await nativeFramePath(contents.mainFrame, first), ['iframe[id=first]']);
  assert.deepEqual(await nativeFramePath(contents.mainFrame, second), ['iframe[id=second]']);
  await first.executeJavaScript(
    'const nested = document.createElement("iframe"); nested.id="nested"; nested.src="about:blank"; document.body.appendChild(nested)',
  );
  const nested = first.frames[0];
  assert.ok(nested);
  assert.equal(nested.processId, first.processId);
  assert.deepEqual(await nativeFramePath(contents.mainFrame, nested), ['iframe[id=first]', 'iframe[id=nested]']);
  assert.equal(await first.executeJavaScript('typeof __oyaNativeRecording'), 'undefined');
  await contents.executeJavaScript('document.querySelector("#first").before(document.querySelector("#second"))');
  second = contents.mainFrame.frames.find((frame) => frame.name === 'second');
  assert.deepEqual(await nativeFramePath(contents.mainFrame, first), ['iframe[id=first]']);
  assert.deepEqual(await nativeFramePath(contents.mainFrame, second), ['iframe[id=second]']);
  await contents.executeJavaScript(
    'document.querySelectorAll("iframe").forEach(frame => { frame.dataset.restoreId=frame.id; frame.removeAttribute("id"); frame.removeAttribute("name"); })',
  );
  await assert.rejects(nativeFramePath(contents.mainFrame, second), /unique stable selector/);
  await contents.executeJavaScript(
    'document.querySelectorAll("iframe").forEach(frame => { frame.id=frame.dataset.restoreId; frame.name=frame.id; })',
  );
  await contents.executeJavaScript(
    `const host = document.createElement('div'); document.body.appendChild(host); const root = host.attachShadow({mode:'open'}); const frame = document.createElement('iframe'); frame.id='shadow'; frame.name='shadow-frame'; frame.src='http://localhost:${server.address().port}/child'; root.appendChild(frame);`,
  );
  for (let i = 0; i < 100 && !contents.mainFrame.frames.some((frame) => frame.name === 'shadow-frame'); i++)
    await new Promise((resolve) => setTimeout(resolve, 20));
  const shadow = contents.mainFrame.frames.find((frame) => frame.name === 'shadow-frame');
  assert.ok(shadow);
  assert.deepEqual(await nativeFramePath(contents.mainFrame, shadow), ['iframe[id=shadow]']);
  assert.deepEqual(await nativeFramePath(contents.mainFrame, second), ['iframe[id=second]']);
  const pending = evaluateFrame(
    first,
    'globalThis.oyaPending=new Promise(()=>{document.body.dataset.oyaPending="yes"})',
  );
  const cancelled = assert.rejects(withDeadline(pending), /execution context was destroyed|disposed/i);
  assert.equal(await evaluateFrame(first, 'document.body.dataset.oyaPending'), 'yes');
  await contents.executeJavaScript('document.querySelector("#first").remove()');
  await cancelled;
  await assert.rejects(evaluateFrame(first, '42'));
  assert.equal(await evaluateFrame(second, '6*7'), 42);
  await pendingNavigation(win);
  await stalledResource(win);
  const pendingOnClose = assert.rejects(
    withDeadline(evaluateFrame(contents.mainFrame, 'globalThis.oyaPending=new Promise(()=>{})')),
    /execution context was destroyed/i,
  );
  win.destroy();
  await pendingOnClose;
  console.log(
    'PASS: Oya native cross-process frames, duplicate URLs, reordered/nested/shadow owners, isolated globals, errors and detached-frame refusal',
  );
}
/** Reload must cancel old-world work, and a never-settling promise must have a native deadline. */
async function pendingNavigation(win) {
  const contents = win.webContents;
  const pending = evaluateFrame(
    contents.mainFrame,
    'globalThis.oldDocument=1; globalThis.oyaPending=new Promise(()=>{document.body.dataset.pending="yes"})',
  );
  const cancelled = assert.rejects(withDeadline(pending), /execution context was destroyed/i);
  assert.equal(await evaluateFrame(contents.mainFrame, 'document.body.dataset.pending'), 'yes');
  await win.loadURL(`http://127.0.0.1:${server.address().port}/child?new`);
  await cancelled;
  assert.equal(await evaluateFrame(contents.mainFrame, 'typeof oldDocument'), 'undefined');
  await assert.rejects(
    evaluateFrame(contents.mainFrame, 'globalThis.oyaPending=new Promise(()=>{})'),
    /Native isolated execution timed out/,
  );
  assert.equal(await evaluateFrame(contents.mainFrame, '6*7'), 42);
  console.log(
    'PASS: navigation cancels native evaluation, stale globals do not transfer, and unresolved promises time out',
  );
}
/** A discarded document must reject its outstanding evaluation, not leave the caller waiting forever. */
async function withDeadline(promise) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error('Pending native execution did not cancel')), 3000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
app
  .whenReady()
  .then(run)
  .then(
    () => finish(0),
    (error) => {
      console.error(error);
      finish(1);
    },
  );
/** A stalled resource cannot prevent inspection of an already committed, usable document. */
async function stalledResource(win) {
  const contents = win.webContents;
  const committed = new Promise((resolve) => contents.once('did-navigate', resolve));
  void win.loadURL(`http://127.0.0.1:${server.address().port}/loading`).catch(() => {});
  await committed;
  const world = new World({ analyzerScript: 'void 0;', worldName: 'loading-test' });
  let value;
  for (let i = 0; i < 100; i++) {
    value = await world.evaluate({ webContents: contents }, 'document.getElementById("ready")?.textContent');
    if (value) break;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.equal(value, 'Ready now');
  assert.equal(contents.isLoadingMainFrame(), true, 'the test must not wait for resource completion');
  console.log('PASS: native World inspection while a parser-blocking resource is stalled');
}
/** Close only test-owned resources, leaving user windows and profiles untouched. */
function finish(code) {
  clearTimeout(deadline);
  server.close();
  fs.rmSync(profile, { recursive: true, force: true });
  app.exit(code);
}
