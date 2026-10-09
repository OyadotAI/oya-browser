/** Native channel owns automatic navigation/frame re-arming and final typing without a debugger backend. */
const assert = require('node:assert/strict');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { evaluateFrame } = require('../../src/main/native/index.ts');
const { recordingPreferences } = require('../../src/main/recording/preload.ts');
const { RecordingChannels } = require('../../src/main/recording/channels.ts');
const { NativeRecordingChannel } = require('../../src/main/recording/native-channel.ts');
/** Bound fixture readiness; the production channel itself uses native events rather than polling. */
async function until(read) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (await read()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw Error('Native recording channel did not become ready');
}
/** Native text composition reaches the actual focused frame, including out-of-process children. */
async function type(page, frame, text) {
  await frame.executeJavaScript('document.querySelector("input").focus(); document.querySelector("input").select()');
  await until(() => page.focusedFrame === frame);
  await page.insertText(text);
  await until(() => frame.executeJavaScript(`document.querySelector('input').value === ${JSON.stringify(text)}`));
}
/** Observe the real isolated recorder, not a synthetic message or private channel field. */
const ready = (frame) => evaluateFrame(frame, 'globalThis.__oyaDocumentRecorder?.armed === true');
/** Actual engine fixture verifies the production channel's full-page lifecycle and strict isolation. */
module.exports = async function checkChannel(origin, analyzer) {
  const window = new BrowserWindow({
    show: true,
    webPreferences: recordingPreferences(path.resolve(__dirname, '../..')),
  });
  const page = window.webContents;
  Object.defineProperty(page, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  const batches = [];
  const channel = new NativeRecordingChannel(page, analyzer, (batch) => batches.push(batch));
  const steps = () => batches.flatMap((batch) => batch.steps || []);
  try {
    await window.loadURL(origin + '/parent');
    app.focus({ steal: true });
    window.focus();
    page.focus();
    await until(() => window.isFocused());
    const first = channel.start();
    assert.equal(channel.start(), first);
    await first;
    const child = page.mainFrame.frames[0];
    assert.notEqual(child.processId, page.mainFrame.processId);
    assert.equal(await ready(child), true);
    await type(page, child, 'native child edit before navigation');
    await window.loadURL(origin + '/replacement');
    await until(() => ready(page.mainFrame));
    await channel.drain(true);
    await until(() => steps().some((step) => step.text === 'native child edit before navigation'));
    const childStep = steps().find((step) => step.text === 'native child edit before navigation');
    assert.equal(childStep.frames.length, 1);
    assert.equal(await page.executeJavaScript('typeof __oyaNativeRecording'), 'undefined');
    await page.executeJavaScript(
      `{const frame=document.createElement('iframe');frame.id='added';frame.src=${JSON.stringify(origin.replace('127.0.0.1', 'localhost') + '/child')};document.body.append(frame);}`,
    );
    await until(() => page.mainFrame.frames.length === 1);
    const added = page.mainFrame.frames[0];
    await until(() => ready(added));
    await type(page, added, 'dynamically added frame');
    await channel.drain(true);
    assert.ok(steps().some((step) => step.text === 'dynamically added frame' && step.frames.length === 1));
    await page.executeJavaScript('document.querySelector("iframe").remove()');
    await type(page, page.mainFrame, 'last edit before stop');
    await channel.stop();
    assert.ok(steps().some((step) => step.text === 'last edit before stop' && step.frames.length === 0));
    assert.equal(
      steps().some((step) => step.captureIssue),
      false,
    );
    assert.equal(await evaluateFrame(page.mainFrame, 'typeof __oyaDocumentRecorder'), 'undefined');
    await assert.rejects(channel.start(), /stopped/);
    const next = new NativeRecordingChannel(page, analyzer, (batch) => batches.push(batch));
    await next.start();
    await next.stop();
    assert.equal(page.listenerCount('ipc-message'), 0);
    const view = { webContents: page };
    const tabs = { list: [{ id: 7, view }] };
    const received = [];
    const manager = new RecordingChannels({
      tabs,
      analyzerScript: analyzer,
      recorder: { receive: (...args) => received.push(args) },
    });
    await manager.armRecordingView(view);
    await type(page, page.mainFrame, 'manager final edit');
    tabs.list = [];
    await manager.stopAll();
    assert.ok(
      received.some(
        ([source, url, out, id]) =>
          source === view &&
          url === origin + '/replacement' &&
          id === 7 &&
          out.steps?.some((step) => step.text === 'manager final edit'),
      ),
    );
    assert.equal(page.listenerCount('ipc-message'), 0);
  } finally {
    await channel.stop().catch(() => {});
    window.destroy();
  }
  console.log(
    'PASS: native recording channel auto-arms navigation and added cross-process frames, captures final typing, isolates frame paths and restarts without leaked listeners',
  );
};
