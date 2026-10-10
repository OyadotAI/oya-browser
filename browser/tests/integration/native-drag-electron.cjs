/** Hermetic native drag/drop behavior, cancellation and ownership regression in real Oya. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { once } = require('node:events');
const { app, BrowserWindow } = require('electron');
const { World, evaluateFrame } = require('../../src/main/native/index.ts');
const { findElementJs } = require('../../src/main/actions/scripts.ts');
const { POINTER_COMMANDS } = require('../../src/main/actions/pointer-commands.ts');
const { nativeDrag } = require('../../src/main/input/index.ts');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-native-drag-'));
app.setPath('userData', path.join(dir, 'profile'));
app.commandLine.appendSwitch('site-per-process');
let frameServer;
const world = new World({
  analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'native-drag-regression',
});
const fixture = path.join(dir, 'fixture.html');
const replacement = path.join(dir, 'replacement.html');
fs.writeFileSync(replacement, '<!doctype html><body>Replacement document</body>');
/** The fixture's own listeners produce the real payload; the test never creates DOM drag events. */
function writeFixture(mode = 'accept') {
  fs.writeFileSync(
    fixture,
    `<!doctype html><style>
    body {margin:30px} .box {display:inline-block;width:140px;height:140px;margin-right:50px;background:#eee}
    input {display:block;width:350px;margin-top:30px}
    </style><div class="box" draggable="true">Source</div><button class="box">Target</button>
    <input type="range" min="0" max="100" value="0"><output></output><script>
    const boxes = document.querySelectorAll('.box'), source=boxes[0], target=boxes[1];
    document.body.dataset.events='';
    function record(e) { document.body.dataset.events += e.type+':'+e.isTrusted+';'; }
    source.addEventListener('dragstart', e => {
      record(e); e.dataTransfer.effectAllowed='move'; e.dataTransfer.setData('text/plain','owned-payload');
      if (${JSON.stringify(mode)} === 'navigate') setTimeout(()=>location.href='replacement.html',30);
      if (${JSON.stringify(mode)} === 'cancel') e.preventDefault();
    });
    source.addEventListener('dragend', e => {record(e); document.body.dataset.effect=e.dataTransfer.dropEffect;});
    target.addEventListener('dragenter', record);
    target.addEventListener('dragover', e => {record(e); if (${JSON.stringify(mode)} !== 'refuse') { e.preventDefault(); e.dataTransfer.dropEffect='move'; }});
    target.addEventListener('drop', e => { record(e); e.preventDefault(); target.textContent=e.dataTransfer.getData('text/plain'); });
    document.querySelector('input').addEventListener('input',e=>document.querySelector('output').textContent=e.isTrusted+':'+e.target.value);
    </script>`,
  );
}
/** Bound test observations independently of the engine's operation deadline. */
async function until(fn) {
  const deadline = Date.now() + 4000;
  while (Date.now() < deadline) {
    if (await fn()) return;
    await new Promise((r) => setTimeout(r, 25));
  }
  throw Error('Native drag observation timed out');
}
/** Resolve coordinates from fresh production analyzer ids. */
async function target(view, text) {
  const result = await world.evaluate(view, 'analyzePage({})');
  assert.equal(result.ok, true);
  const el = result.data.elements.find((e) => e.text === text);
  assert.ok(el, 'Missing analyzer target ' + text);
  const point = await world.evaluate(view, findElementJs(String(el.id)));
  assert.equal(point.ok, true);
  return { x: point.data.x, y: point.data.y };
}
/** Open and focus only the test's own window; no global keystrokes or accessibility automation. */
async function load(win, mode = 'accept') {
  writeFixture(mode);
  await win.loadFile(fixture);
  win.show();
  win.focus();
  win.webContents.focus();
  await until(() => win.isFocused() && win.webContents.isFocused());
  const view = { webContents: win.webContents };
  return { view, from: await target(view, 'Source'), to: await target(view, 'Target') };
}
/** One action runs through the production command facade, not a test-specific input implementation. */
async function command({ view, from, to }) {
  let answer;
  await POINTER_COMMANDS.drag(
    {
      activeView: () => view,
      deps: {
        sendResult: (_id, ok, data, error) => {
          assert.equal(ok, true, error);
          answer = data;
        },
      },
    },
    'drag',
    { from_x: from.x, from_y: from.y, to_x: to.x, to_y: to.y },
  );
  assert.equal(answer.kind, 'html');
}
/** Validate real browser-produced events and unchanged renderer-owned drag data. */
async function accepted(view) {
  await until(async () => (await world.evaluate(view, 'document.body.dataset.events')).includes('dragend:true'));
  assert.equal(await world.evaluate(view, 'document.querySelectorAll(".box")[1].textContent'), 'owned-payload');
  const events = await world.evaluate(view, 'document.body.dataset.events');
  for (const type of ['dragstart', 'dragenter', 'dragover', 'drop', 'dragend'])
    assert.ok(events.includes(type + ':true'), events);
  assert.ok(!events.includes(':false'), events);
  assert.equal(await world.evaluate(view, 'document.body.dataset.effect'), 'move');
}
const deadline = setTimeout(() => {
  console.error('Native drag suite timed out');
  app.exit(1);
}, 60000);
app
  .whenReady()
  .then(async () => {
    const win = new BrowserWindow({
      width: 900,
      height: 650,
      webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
    });
    Object.defineProperty(win.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    let qa = await load(win);
    assert.equal(typeof win.webContents._dragOya, 'function', 'Missing native drag engine capability');
    await command(qa);
    await accepted(qa.view);
    console.log('PASS: production drag uses trusted events and renderer-owned payload');

    qa = await load(win, 'refuse');
    await assert.rejects(nativeDrag(qa.view, qa.from, qa.to), /did not accept/);
    assert.equal(await world.evaluate(qa.view, 'document.querySelectorAll(".box")[1].textContent'), 'Target');
    qa = await load(win);
    await command(qa);
    await accepted(qa.view);
    console.log('PASS: refused drop fails and releases state for the next drag');

    qa = await load(win);
    const first = nativeDrag(qa.view, qa.from, qa.to);
    await assert.rejects(nativeDrag(qa.view, qa.from, qa.to), /already active/);
    assert.equal(await first, 'html');
    await accepted(qa.view);
    await assert.rejects(nativeDrag(qa.view, { x: -1, y: 0 }, qa.to), /inside the owning viewport/);
    console.log('PASS: concurrent and out-of-bounds gestures are refused');

    qa = await load(win, 'navigate');
    const replacementLoaded = once(win.webContents, 'did-finish-load');
    await assert.rejects(nativeDrag(qa.view, qa.from, qa.to), /navigation|document/);
    await replacementLoaded;
    await until(() => win.webContents.getURL().endsWith('replacement.html'));
    assert.equal(await world.evaluate(qa.view, 'document.body.innerText'), 'Replacement document');
    console.log('PASS: navigation cancels the gesture without replaying it in the next document');

    writeFixture();
    const frameHost = path.join(dir, 'frame-host.html');
    fs.writeFileSync(frameHost, '<iframe src="fixture.html" style="width:750px;height:400px"></iframe>');
    await win.loadFile(frameHost);
    win.focus();
    win.webContents.focus();
    const frameView = { webContents: win.webContents };
    const childFrom = await target(frameView, 'Source'),
      childTo = await target(frameView, 'Target');
    await assert.rejects(nativeDrag(frameView, childFrom, childTo), /Unsupported native drag (source|target)/);
    assert.equal(
      await world.evaluate(
        frameView,
        'document.querySelector("iframe").contentDocument.querySelectorAll(".box")[1].textContent',
      ),
      'Target',
    );
    console.log('PASS: unsupported child-frame drags fail without crossing document ownership');

    frameServer = http.createServer((_request, response) => {
      response.setHeader('Content-Type', 'text/html');
      response.end(
        '<body ondragover="event.preventDefault()" ondrop="document.body.dataset.dropped=event.isTrusted">Foreign target</body>',
      );
    });
    await new Promise((resolve) => frameServer.listen(0, '127.0.0.1', resolve));
    const frameURL = `http://127.0.0.1:${frameServer.address().port}/`;
    const crossHost = path.join(dir, 'cross-host.html');
    writeFixture();
    fs.writeFileSync(
      crossHost,
      fs.readFileSync(fixture, 'utf8') +
        `<iframe src="${frameURL}" style="position:absolute;left:260px;top:30px;width:200px;height:160px"></iframe>`,
    );
    await win.loadFile(crossHost);
    win.focus();
    win.webContents.focus();
    const remote = win.webContents.mainFrame.frames.find((frame) => frame.url === frameURL);
    assert.ok(remote);
    assert.notEqual(remote.processId, win.webContents.mainFrame.processId, 'Fixture must use a real separate renderer');
    const remoteFrom = await target(frameView, 'Source');
    await assert.rejects(nativeDrag(frameView, remoteFrom, { x: 360, y: 100 }), /crossing renderer widgets/);
    assert.equal(await evaluateFrame(remote, 'Object.hasOwn(document.body.dataset, "dropped")'), false);
    await new Promise((resolve) => frameServer.close(resolve));
    frameServer = null;
    console.log("PASS: a real out-of-process target cannot receive another document's drag payload");

    qa = await load(win);
    const pending = nativeDrag(qa.view, qa.from, qa.to);
    const rejection = assert.rejects(pending, /focused document/);
    const other = new BrowserWindow({ width: 300, height: 200 });
    other.show();
    other.focus();
    other.webContents.focus();
    await rejection;
    other.destroy();
    qa = await load(win);
    await command(qa);
    await accepted(qa.view);
    console.log('PASS: focus loss cancels instead of routing the drag into another window');

    qa = await load(win, 'cancel');
    assert.equal(await nativeDrag(qa.view, qa.from, qa.to), 'pointer');
    assert.ok(!(await world.evaluate(qa.view, 'document.body.dataset.events')).includes('drop:'));
    console.log('PASS: page-cancelled dragstart is not reported as an HTML drop');

    qa = await load(win);
    const range = await world.evaluate(
      qa.view,
      '(()=>{const r=document.querySelector("input").getBoundingClientRect();return {x:r.x+8,y:r.y+r.height/2,end:r.right-8};})()',
    );
    assert.equal(await nativeDrag(qa.view, { x: range.x, y: range.y }, { x: range.end, y: range.y }), 'pointer');
    await until(async () =>
      /^true:([8-9][0-9]|100)$/.test(await world.evaluate(qa.view, 'document.querySelector("output").textContent')),
    );
    console.log('PASS: ordinary slider dragging retains trusted native input');

    qa = await load(win);
    const closing = assert.rejects(nativeDrag(qa.view, qa.from, qa.to), /destroyed|document/);
    win.destroy();
    await closing;
    console.log('PASS: destroying the source window rejects its pending gesture');
    clearTimeout(deadline);
    app.exit(0);
  })
  .catch((error) => {
    console.error(error);
    frameServer?.close();
    clearTimeout(deadline);
    app.exit(1);
  });
