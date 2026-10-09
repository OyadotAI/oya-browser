/** Native dialog callbacks in Oya: no debugger, injected shims or automatic decision acceptance. */
const assert = require('node:assert/strict');
const { once } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { NativeDialogs } = require('../../src/main/dialogs/index.ts');
const { watchNativeDialogs } = require('../../src/main/native/index.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-native-dialogs-'));
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Native dialog test timed out');
  app.exit(1);
}, 60000);
/** Resolve only after a native dialog callback has actually arrived. */
function receiver() {
  let deliver;
  const next = new Promise((resolve) => {
    deliver = resolve;
  });
  return { next, handle: (info, reply) => deliver({ info, reply }) };
}
/** The actual command-facing service queues independent native renderer dialogs. */
async function checkService(fixture) {
  const dialogs = new NativeDialogs();
  const windows = [new BrowserWindow({ show: false }), new BrowserWindow({ show: false })];
  for (const win of windows) {
    Object.defineProperty(win.webContents, 'debugger', {
      get() {
        throw new Error('Internal CDP is forbidden');
      },
    });
    await win.loadFile(fixture);
    dialogs.watch(win.webContents);
  }
  const firstOpened = once(windows[0].webContents, '-run-dialog');
  const first = windows[0].webContents.executeJavaScript('confirm("First tab")');
  await firstOpened;
  const secondOpened = once(windows[1].webContents, '-run-dialog');
  const second = windows[1].webContents.executeJavaScript('prompt("Second tab", "default")');
  await secondOpened;
  assert.equal(dialogs.current().message, 'First tab');
  await dialogs.answer(false);
  assert.equal(await first, false);
  assert.equal(dialogs.current().message, 'Second tab');
  await dialogs.answer(true, 'explicit answer');
  assert.equal(await second, 'explicit answer');
  assert.equal(dialogs.current(), null);
  for (const win of windows) {
    dialogs.unwatch(win.webContents);
    win.destroy();
  }
}
/** Disposable pages keep normal human windows and profiles untouched. */
async function run() {
  const win = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true } });
  const wc = win.webContents;
  wc.on('console-message', (event) => console.error('Renderer:', event.message));
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw new Error('Internal CDP is forbidden');
    },
  });
  const fixture = path.join(profile, 'dialogs.html');
  fs.writeFileSync(fixture, '<!doctype html><title>Native dialogs</title>');
  await win.loadFile(fixture);
  let receive = receiver();
  const cancelled = [];
  const stop = watchNativeDialogs(
    wc,
    (info, reply) => receive.handle(info, reply),
    (reply) => cancelled.push(reply),
  );
  const confirm = wc.executeJavaScript('confirm("Delete this?")');
  confirm.catch((error) => {
    console.error(error);
    finish(1);
  });
  const decision = await receive.next;
  assert.equal(decision.info.dialogType, 'confirm');
  assert.equal(decision.info.messageText, 'Delete this?');
  assert.equal(decision.info.frame, wc.mainFrame);
  assert.throws(stop, /pending/);
  decision.reply(false);
  decision.reply(true);
  assert.equal(await confirm, false);
  console.log('confirm passed');
  receive = receiver();
  const prompt = wc.executeJavaScript('prompt("Name?", "Original")');
  prompt.catch((error) => {
    console.error(error);
    finish(1);
  });
  const question = await receive.next;
  assert.equal(question.info.defaultPromptText, 'Original');
  question.reply(true, 'Native answer');
  assert.equal(await prompt, 'Native answer');
  console.log('prompt passed');
  receive = receiver();
  const alert = wc.executeJavaScript('alert("Saved"); 42');
  const notice = await receive.next;
  assert.equal(notice.info.dialogType, 'alert');
  notice.reply(true);
  assert.equal(await alert, 42);
  console.log('alert passed');
  receive = receiver();
  void wc.executeJavaScript('confirm("Leaving?")').catch(() => undefined);
  const stale = await receive.next;
  await win.loadURL('data:text/html,<title>Replacement</title>');
  console.log('navigation committed');
  assert.ok(cancelled.includes(stale.reply));
  stale.reply(true);
  assert.equal(await wc.executeJavaScript('6 * 7'), 42);
  stop();
  receive = receiver();
  const again = watchNativeDialogs(
    wc,
    (info, reply) => receive.handle(info, reply),
    (reply) => cancelled.push(reply),
  );
  stop(); // An old disposer cannot clear the new subscription.
  void wc.executeJavaScript('confirm("Closing window?")').catch(() => undefined);
  const closing = await receive.next;
  const destroyed = once(wc, 'destroyed');
  win.destroy();
  await destroyed;
  assert.ok(cancelled.includes(closing.reply));
  closing.reply(true);
  again();
  const disabled = new BrowserWindow({ show: false, webPreferences: { sandbox: true, disableDialogs: true } });
  Object.defineProperty(disabled.webContents, 'debugger', {
    get() {
      throw new Error('Internal CDP is forbidden');
    },
  });
  await disabled.loadFile(fixture);
  watchNativeDialogs(
    disabled.webContents,
    () => assert.fail('Disabled dialogs must remain disabled'),
    () => {},
  );
  assert.equal(await disabled.webContents.executeJavaScript('confirm("Not allowed")'), false);
  disabled.destroy();
  await checkService(fixture);
  await require('./native-before-unload.cjs')(profile);
  console.log(
    'PASS: Oya native confirm/prompt/alert, exact frame, single-use replies, navigation cancellation and teardown; debugger forbidden',
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
/** Remove only the owned fixture profile. */
function finish(code) {
  clearTimeout(deadline);
  fs.rmSync(profile, { recursive: true, force: true });
  app.exit(code);
}
