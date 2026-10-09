/** Actual native unload decisions preserve user data and never use a debugging backend. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { once } = require('node:events');
const { app, BrowserWindow } = require('electron');
const { watchNativeDialogs } = require('../../src/main/native/index.ts');
/** Observe native input delivery without assuming an arbitrary paint delay. */
async function until(read) {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (await read()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw Error('Native unload condition did not arrive');
}
/** A real trusted click activates the document's before-unload protection. */
async function activate(window, fixture) {
  await window.loadFile(fixture);
  window.show();
  app.focus({ steal: true });
  window.focus();
  const page = window.webContents;
  page.focus();
  await until(() => window.isFocused());
  page.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, x: 40, y: 40 });
  page.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, x: 40, y: 40 });
  await until(() => page.executeJavaScript('window.activated === true'));
}
/** Only the current native reply may decide a pending navigation. */
async function navigation(window, fixture, receive) {
  const page = window.webContents;
  await activate(window, fixture);
  const refused = window.loadURL('data:text/html,<title>Discarded</title>').then(
    () => null,
    (error) => error,
  );
  const first = await receive();
  assert.equal(first.info.dialogType, 'beforeunload');
  assert.equal(first.info.isReload, false);
  assert.equal(first.info.frame, page.mainFrame);
  first.reply(false);
  assert.match((await refused).message, /ERR_ABORTED/);
  assert.equal(await page.executeJavaScript('document.title'), 'Unsaved');
  const allowed = window.loadURL('data:text/html,<title>Accepted</title>');
  const second = await receive();
  first.reply(true); // A stale decision cannot accept the next navigation.
  second.reply(true);
  await allowed;
  assert.equal(await page.executeJavaScript('document.title'), 'Accepted');
}
/** Reload and close both honor a refused decision and permit a later explicit acceptance. */
async function reloadAndClose(window, fixture, receive) {
  const page = window.webContents;
  await activate(window, fixture);
  page.reload();
  const denied = await receive();
  assert.equal(denied.info.isReload, true);
  denied.reply(false);
  assert.equal(await page.executeJavaScript('window.activated'), true);
  const loaded = once(page, 'did-finish-load');
  page.reload();
  const accepted = await receive();
  accepted.reply(true);
  await loaded;
  assert.equal(await page.executeJavaScript('window.activated'), false);
  await activate(window, fixture);
  window.close();
  const stay = await receive();
  stay.reply(false);
  assert.equal(await page.executeJavaScript('document.title'), 'Unsaved');
  assert.equal(window.isDestroyed(), false);
  const closed = once(window, 'closed');
  window.close();
  const leave = await receive();
  leave.reply(true);
  await closed;
}
/** Unsubscribing restores Electron's original prevent-unload event, not an unattended accept. */
async function legacyAndCancellation(window, fixture, receive, cancelled, stop) {
  const page = window.webContents;
  await activate(window, fixture);
  stop();
  const prevented = once(page, 'will-prevent-unload');
  const refused = window.loadURL('about:blank').then(
    () => null,
    (error) => error,
  );
  await prevented;
  assert.match((await refused).message, /ERR_ABORTED/);
  assert.equal(await page.executeJavaScript('document.title'), 'Unsaved');
  const dispose = watchNativeDialogs(page, receive.handler, (reply) => cancelled.push(reply));
  const loading = window.loadURL('about:blank').catch(() => {});
  const pending = await receive();
  assert.throws(dispose, /pending/);
  const destroyed = once(page, 'destroyed');
  window.destroy();
  await destroyed;
  assert.ok(cancelled.includes(pending.reply));
  pending.reply(true);
  dispose();
  await loading;
}
/** Queue arrival is independent from initiating loadURL's promise, which waits for the decision. */
function receiver() {
  const pending = [];
  const queued = [];
  const receive = () =>
    queued.length ? Promise.resolve(queued.shift()) : new Promise((resolve) => pending.push(resolve));
  receive.handler = (info, reply) => {
    const decision = { info, reply };
    if (pending.length) pending.shift()(decision);
    else queued.push(decision);
  };
  return receive;
}
/** Isolated local fixtures exercise Blink's actual before-unload and cancellation lifecycle. */
module.exports = async function checkBeforeUnload(profile) {
  const fixture = path.join(profile, 'unload.html');
  fs.writeFileSync(
    fixture,
    '<!doctype html><title>Unsaved</title><style>body{margin:0}button{width:160px;height:100px}</style><button>Activate</button><script>window.activated=false;document.querySelector("button").onclick=e=>{window.activated=e.isTrusted};onbeforeunload=e=>{e.preventDefault();e.returnValue=""}</script>',
  );
  for (const check of ['decisions', 'cancellation']) {
    const window = new BrowserWindow({ width: 500, height: 350, show: false, webPreferences: { sandbox: true } });
    Object.defineProperty(window.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    const receive = receiver();
    const cancelled = [];
    const stop = watchNativeDialogs(window.webContents, receive.handler, (reply) => cancelled.push(reply));
    try {
      if (check === 'decisions') {
        await navigation(window, fixture, receive);
        await reloadAndClose(window, fixture, receive);
      } else await legacyAndCancellation(window, fixture, receive, cancelled, stop);
    } finally {
      if (!window.isDestroyed()) window.destroy();
      stop();
    }
  }
  console.log(
    'PASS: native before-unload navigation, reload, close, cancellation and human fallback; debugger forbidden',
  );
};
