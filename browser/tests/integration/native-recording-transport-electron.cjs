/** Native recording transport: isolated IPC survives navigation and works in cross-process child frames. */
const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { evaluateFrame, NativeRecordingInbox, NativeDocumentRecorder } = require('../../src/main/native/index.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-record-transport-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Native recording transport timed out');
  app.exit(1);
}, 30000);
const server = http.createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(
    req.url === '/parent'
      ? `<input id="value" value="parent"><iframe src="http://localhost:${server.address().port}/child"></iframe>`
      : '<input id="value" value="child">',
  );
});
/** A short bounded wait observes IPC arrival, not a production polling transport. */
async function until(read) {
  for (let i = 0; i < 100; i++) {
    if (await read()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error('Native recording batch did not arrive');
}
/** Nothing in the page main world may reach Node or the isolated recording capability. */
async function assertIsolated(frame) {
  assert.deepEqual(await frame.executeJavaScript('[typeof __oyaNativeRecording, typeof require, typeof process]'), [
    'undefined',
    'undefined',
    'undefined',
  ]);
  assert.equal(await evaluateFrame(frame, 'typeof __oyaNativeRecording.emit'), 'function');
  assert.equal(await evaluateFrame(frame, 'typeof require'), 'undefined');
}
/** Cross-origin frames emit directly through native IPC, never a website-observable postMessage bridge. */
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
  const wc = win.webContents;
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw new Error('Internal CDP is forbidden');
    },
  });
  const batches = [];
  const inbox = new NativeRecordingInbox(wc, ({ frame, payload, document }) =>
    batches.push({ frame, payload: JSON.parse(payload), document }),
  );
  const epoch = inbox.start();
  await win.loadURL(`http://127.0.0.1:${server.address().port}/parent`);
  const parent = wc.mainFrame;
  const child = parent.frames[0];
  assert.notEqual(child.processId, parent.processId);
  await assertIsolated(parent);
  await assertIsolated(child);
  const parentDocument = await inbox.authorize(parent);
  const childDocument = await inbox.authorize(child);
  assert.notEqual(parentDocument.documentId, childDocument.documentId);
  assert.equal(childDocument.frames.length, 1);
  for (const [frame, name] of [
    [parent, 'parent'],
    [child, 'child'],
  ]) {
    await evaluateFrame(
      frame,
      `__oyaNativeRecording.emit(${JSON.stringify(epoch)}, JSON.stringify({kind:'ready', name:${JSON.stringify(name)}}));
      addEventListener('pagehide', () => __oyaNativeRecording.emit(${JSON.stringify(epoch)}, JSON.stringify({kind:'final', name:${JSON.stringify(name)}, value:document.querySelector('input').value})), {once:true});`,
    );
  }
  await until(() => batches.length === 2);
  assert.equal(batches.find((batch) => batch.payload.name === 'parent').frame, parent);
  assert.equal(batches.find((batch) => batch.payload.name === 'child').frame, child);
  const analyzer = fs
    .readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8')
    .replace('__OYA_ATTR__', 'data-native-transport-test')
    .replace('__OYA_RECORD__', 'false');
  const childRecorder = new NativeDocumentRecorder(childDocument, epoch, analyzer);
  await childRecorder.start();
  assert.deepEqual((await childRecorder.drain()).steps, []);
  win.show();
  win.focus();
  wc.focus();
  await until(() => win.isFocused());
  await child.executeJavaScript('document.querySelector("input").focus(); document.querySelector("input").select()');
  await until(() => wc.focusedFrame === child);
  await until(() => child.executeJavaScript('document.hasFocus() && document.activeElement.id === "value"'));
  await wc.insertText('last edit before navigation');
  await until(() => child.executeJavaScript('document.querySelector("input").value === "last edit before navigation"'));
  await win.loadURL(`http://127.0.0.1:${server.address().port}/replacement`);
  await until(() => batches.filter((batch) => batch.payload.kind === 'final').length === 2);
  assert.equal(
    batches.find((batch) => batch.payload.kind === 'final' && batch.payload.name === 'child').payload.value,
    'last edit before navigation',
  );
  await until(() =>
    batches.some((batch) =>
      batch.payload.steps?.some((step) => step.action === 'type' && step.text === 'last edit before navigation'),
    ),
  );
  const recorded = batches.find((batch) => batch.payload.steps?.some((step) => step.action === 'type'));
  assert.equal(recorded.frame, child);
  assert.equal(recorded.document, childDocument);
  assert.equal(recorded.document.url, `http://localhost:${server.address().port}/child`);
  assert.equal(recorded.document.frames.length, 1);
  const replacement = wc.mainFrame;
  const count = batches.length;
  await evaluateFrame(replacement, `__oyaNativeRecording.emit(${JSON.stringify(epoch)}, '{}')`);
  await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal(batches.length, count, 'unarmed replacement document must not deliver');
  const replacementDocument = await inbox.authorize(replacement);
  assert.notEqual(replacementDocument.documentId, parentDocument.documentId);
  await evaluateFrame(
    replacement,
    `__oyaNativeRecording.emit(${JSON.stringify(epoch)}, JSON.stringify({kind:'replacement'}))`,
  );
  await until(() => batches.some((batch) => batch.payload.kind === 'replacement'));
  assert.equal(batches.at(-1).document, replacementDocument);
  await assert.rejects(childRecorder.drain());
  await assert.rejects(childRecorder.stop());
  const replacementRecorder = new NativeDocumentRecorder(replacementDocument, epoch, analyzer);
  await replacementRecorder.start();
  await replacement.executeJavaScript(
    'document.querySelector("input").focus(); document.querySelector("input").select()',
  );
  await until(() => wc.focusedFrame === replacement);
  await wc.insertText('final edit on stop');
  await until(() => replacement.executeJavaScript('document.querySelector("input").value === "final edit on stop"'));
  const stopped = await replacementRecorder.stop();
  assert.ok(stopped.steps.some((step) => step.action === 'type' && step.text === 'final edit on stop'));
  await assert.rejects(replacementRecorder.start(), /stopped/);
  assert.equal(await evaluateFrame(replacement, 'typeof __oyaDocumentRecorder'), 'undefined');
  await assertIsolated(wc.mainFrame);
  inbox.stop();
  win.destroy();
  console.log(
    'PASS: Oya native isolated recording IPC, cross-process frames, no page/Node exposure, document-scoped lifecycle, navigation flush and final stop drain',
  );
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
/** Clean up only this test's server and disposable profile. */
function finish(code) {
  clearTimeout(deadline);
  server.close();
  fs.rmSync(profile, { recursive: true, force: true });
  app.exit(code);
}
