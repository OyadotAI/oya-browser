/** Exercise Oya's real production human/agent dialog router and isolated UI using native input only. */
const assert = require('node:assert/strict');
const path = require('node:path');
const os = require('node:os');
const fs = require('node:fs');
const electron = require('electron');
const { Protection } = require('../../src/main/tabs/protection.ts');
const { NATIVE_DIALOG } = require('../../src/shared/native-dialog.ts');
const { DesktopDialogs, dialogPresenter } = require('../../src/main/dialogs/index.ts');
/** State waits have a short bound and do not depend on arbitrary animation delays. */
async function until(read) {
  for (let attempt = 0; attempt < 100; attempt++) {
    const value = await read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw Error('Native human dialog UI did not become ready');
}
/** Only the dedicated browser sheet is eligible; website titles never select a target. */
function sheetFor(parent) {
  return electron.BrowserWindow.getAllWindows().find(
    (window) => window.getParentWindow() === parent && window.isModal(),
  );
}
/** Ensure the complete rendered dialog, not a blank shell, is shown and keyboard-focused. */
async function sheet(parent) {
  const window = await until(() => sheetFor(parent)?.isVisible() && sheetFor(parent));
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  electron.app.focus({ steal: true });
  window.focus();
  window.webContents.focus();
  await until(() => window.isFocused());
  assert.equal(await window.webContents.executeJavaScript('document.querySelector("form").hidden'), false);
  return window;
}
/** Native key events verify the actual keyboard-first UI rather than calling its reply handler. */
function key(window, keyCode, modifiers = []) {
  window.webContents.sendInputEvent({ type: 'keyDown', keyCode, modifiers });
  if (keyCode === 'Enter') window.webContents.sendInputEvent({ type: 'char', keyCode: '\r' });
  window.webContents.sendInputEvent({ type: 'keyUp', keyCode, modifiers });
}
/** A typed prompt preserves literal markup and never shares its website's session. */
async function prompt(window) {
  const page = window.webContents;
  const result = page.executeJavaScript('prompt("<img src=x onerror=alert(1)> Name?", "Original")');
  const ui = await sheet(window);
  assert.notEqual(ui.webContents.session, page.session);
  ui.webContents.ipc.emit(NATIVE_DIALOG.ANSWER, { senderFrame: page.mainFrame }, true, 'forged');
  ui.webContents.ipc.emit(NATIVE_DIALOG.ANSWER, { senderFrame: ui.webContents.mainFrame }, 'true', 'forged');
  assert.equal(ui.isDestroyed(), false);
  const state = await ui.webContents.executeJavaScript(
    `({message:document.querySelector('#message').textContent, images:document.images.length, field:document.activeElement.id, node:typeof require, bridge:typeof window.oya, selected:document.querySelector('#value').selectionEnd})`,
  );
  assert.deepEqual(state, {
    message: '<img src=x onerror=alert(1)> Name?',
    images: 0,
    field: 'value',
    node: 'undefined',
    bridge: 'undefined',
    selected: 8,
  });
  await ui.webContents.insertText('Native human answer 日本');
  key(ui, 'Enter');
  assert.equal(await result, 'Native human answer 日本');
  assert.equal(ui.isDestroyed(), true);
}
/** Escape and Enter on the default Cancel button both decline destructive confirmations. */
async function safeDefaults(window) {
  for (const press of ['Escape', 'Enter']) {
    const result = window.webContents.executeJavaScript('confirm("Delete this item?")');
    const ui = await sheet(window);
    assert.equal(await ui.webContents.executeJavaScript('document.activeElement.id'), 'cancel');
    key(ui, press);
    assert.equal(await result, false);
  }
}
/** Ownership changes close or open sheets without making a decision; stale presentation cannot win. */
async function transfer(window, dialogs, setHuman) {
  const result = window.webContents.executeJavaScript('confirm("Human to agent")');
  const ui = await sheet(window);
  setHuman(false);
  dialogs.controlChanged();
  assert.equal(ui.isDestroyed(), true);
  assert.equal(dialogs.current().type, 'confirm');
  await dialogs.answer(false);
  assert.equal(await result, false);
  const next = window.webContents.executeJavaScript('confirm("Agent to human")');
  await until(() => dialogs.current());
  assert.equal(sheetFor(window), undefined);
  setHuman(true);
  dialogs.controlChanged();
  const human = await sheet(window);
  assert.equal((await dialogs.answer(true)).ok, false);
  key(human, 'Escape');
  assert.equal(await next, false);
}
/** A long untrusted message scrolls without hiding its decision buttons. */
async function layout(window) {
  const result = window.webContents.executeJavaScript('prompt("Long message ".repeat(300), "value")');
  const ui = await sheet(window);
  const bounds = await ui.webContents.executeJavaScript(
    `({bottom:document.querySelector('footer').getBoundingClientRect().bottom,height:innerHeight,scroll:document.querySelector('#message').scrollHeight,visible:document.querySelector('#message').clientHeight})`,
  );
  assert.ok(bounds.bottom <= bounds.height);
  assert.ok(bounds.scroll > bounds.visible);
  const snapshot = await ui.webContents.capturePage();
  fs.writeFileSync(path.join(os.tmpdir(), 'oya-native-dialog-sheet.png'), snapshot.toPNG());
  key(ui, 'Escape');
  assert.equal(await result, null);
}
/** Human unload warnings retain unsaved work on Enter and only leave after selecting that action. */
async function unsaved(window) {
  await window.loadURL(
    'data:text/html,<style>body{margin:0}button{width:160px;height:100px}</style><button>Activate</button><script>onbeforeunload=e=>{e.preventDefault();e.returnValue=""};window.active=false;document.querySelector("button").onclick=e=>window.active=e.isTrusted</script>',
  );
  electron.app.focus({ steal: true });
  window.focus();
  window.webContents.focus();
  window.webContents.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, x: 40, y: 40 });
  window.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, x: 40, y: 40 });
  await until(() => window.webContents.executeJavaScript('window.active'));
  const refused = window.loadURL('about:blank').then(
    () => null,
    (error) => error,
  );
  const stay = await sheet(window);
  assert.equal(
    await stay.webContents.executeJavaScript('document.querySelector("#cancel").textContent'),
    'Stay on page',
  );
  key(stay, 'Enter');
  assert.match((await refused).message, /ERR_ABORTED/);
  assert.equal(await window.webContents.executeJavaScript('window.active'), true);
  const accepted = window.loadURL('about:blank');
  const leave = await sheet(window);
  key(leave, 'Tab', ['shift']);
  await until(() => leave.webContents.executeJavaScript('document.activeElement.id === "accept"'));
  key(leave, 'Enter');
  await accepted;
}
/** Same-address and different-address replacement cannot wait on an orphaned modal sheet. */
async function cancellation(window) {
  for (const url of ['about:blank', 'data:text/html,<title>Replacement</title>']) {
    void window.webContents.executeJavaScript('confirm("Going away")').catch(() => {});
    const ui = await sheet(window);
    await window.loadURL(url);
    assert.equal(ui.isDestroyed(), true);
  }
}
/** Actual tab BrowserViews resolve the sheet's parent independently of standalone OAuth windows. */
async function tabSurface() {
  const parent = new electron.BrowserWindow({ show: true, width: 520, height: 400 });
  const view = new electron.BrowserView({ webPreferences: { sandbox: true, contextIsolation: true } });
  parent.addBrowserView(view);
  view.setBounds({ x: 0, y: 0, width: 520, height: 350 });
  Object.defineProperty(view.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  const windows = {
    ownerOfContents: (page) => (page === view.webContents ? { shell: { window: parent } } : undefined),
  };
  const dialogs = new DesktopDialogs(
    () => true,
    dialogPresenter({ electron, windows, appDir: path.resolve(__dirname, '../..') }),
  );
  const protection = new Protection({ nativeBrowsing: true, dialogs });
  try {
    assert.equal(await protection.setupTabCDP(view), true);
    protection.resetTabCDP(view);
    assert.equal(await protection.setupTabCDP(view), true);
    await view.webContents.loadURL('about:blank');
    const result = view.webContents.executeJavaScript('prompt("From a real tab", "default")');
    const ui = await sheet(parent);
    assert.equal(ui.getParentWindow(), parent);
    key(ui, 'Escape');
    assert.equal(await result, null);
  } finally {
    view.webContents.close();
    parent.destroy();
  }
}
/** Production wiring uses the actual Oya engine, fresh page, private sheet session and packaged preload. */
module.exports = async function checkHumanDialogs() {
  const window = new electron.BrowserWindow({ show: true, webPreferences: { sandbox: true, contextIsolation: true } });
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  let human = true;
  const present = dialogPresenter({ electron, appDir: path.resolve(__dirname, '../..') });
  const dialogs = new DesktopDialogs(() => human, present);
  dialogs.watch(window.webContents);
  try {
    await window.loadURL('about:blank');
    await prompt(window);
    console.log('Human prompt passed');
    await safeDefaults(window);
    console.log('Safe defaults passed');
    console.log('Ownership transfer start');
    await transfer(window, dialogs, (value) => {
      human = value;
    });
    await layout(window);
    console.log('Long message layout passed');
    await unsaved(window);
    await cancellation(window);
  } finally {
    window.destroy();
  }
  await tabSurface();
  console.log(
    'PASS: production native dialog UI, trusted keyboard, literal text, private session, safe defaults, ownership transfer, layout and cancellation',
  );
};
