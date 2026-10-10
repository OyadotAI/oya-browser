/** Boot the actual production bundle and exercise authenticated native agent commands with every debugger getter fatal. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { randomBytes } = require('node:crypto');
const { once } = require('node:events');
const { WebSocket } = require('ws');
const { app, BrowserWindow, dialog } = require('electron');
const profile = process.env.OYA_NATIVE_APP_PROFILE;
if (!profile) throw Error('Run native-app.mjs so the parent owns disposable profile cleanup');
const root = path.resolve(__dirname, '../..');
const token = randomBytes(32).toString('hex');
process.env.OYA_USER_DATA_DIR = profile;
process.env.OYA_AUTO_CONNECT = 'false';
process.env.OYA_NATIVE_CDP_PORT = '0';
process.env.OYA_NATIVE_CDP_TOKEN = token;
app.getAppPath = () => root;
fs.writeFileSync(path.join(profile, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));
const servers = [],
  createServer = http.createServer;
http.createServer = (...args) => {
  const server = createServer(...args);
  servers.push(server);
  return server;
};
app.on('web-contents-created', (_event, contents) => {
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw Error('Production native application accessed internal CDP');
    },
  });
});
let prompts = 0,
  answerPrompt;
dialog.showMessageBox = async (_window, options) => {
  assert.equal(options.message, 'Take control of this page?');
  prompts++;
  return new Promise((resolve) => {
    answerPrompt = resolve;
  });
};
const timeout = setTimeout(() => {
  console.error('Native application boot timed out');
  app.exit(1);
}, 60000);
require('../../out/main/index.js');

/** Poll only application-owned readiness, never a signed-in profile or an external website. */
async function until(read) {
  for (let attempt = 0; attempt < 250; attempt++) {
    const value = await read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 40));
  }
  throw Error('Native application readiness failed');
}
/** Find the app's real authenticated native listener, without a fabricated backend or fixed port. */
async function endpoint() {
  for (const server of servers) {
    const port = server.address()?.port;
    if (!port) continue;
    const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
      headers: { authorization: `Bearer ${token}` },
    }).catch(() => null);
    if (!response?.ok) continue;
    const data = await response.json().catch(() => null);
    if (data?.Browser === 'Oya/native') return data.webSocketDebuggerUrl;
  }
}
/** Protocol frames cross the authenticated production front door; no internal backend is exposed to the test. */
async function connect(url) {
  const socket = new WebSocket(url, { headers: { authorization: `Bearer ${token}` } });
  await once(socket, 'open');
  const pending = new Map();
  let sequence = 0;
  socket.on('message', (raw) => {
    const message = JSON.parse(raw.toString()),
      waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    if (message.error) waiter.reject(Error(waiter.method + ': ' + message.error.message));
    else waiter.resolve(message.result);
  });
  return {
    socket,
    call: (method, params = {}, sessionId) =>
      new Promise((resolve, reject) => {
        const id = ++sequence;
        pending.set(id, { resolve, reject, method });
        socket.send(JSON.stringify({ id, method, params, sessionId }));
      }),
  };
}
/** Normal first-run shell, native protection, page input, screenshot and tab retirement run as shipped. */
async function run() {
  await app.whenReady();
  assert.equal(app.commandLine.hasSwitch('remote-debugging-port'), false);
  const shell = await until(() =>
    BrowserWindow.getAllWindows().find((win) => win.webContents.getURL().includes('/out/renderer/index.html')),
  );
  await until(() =>
    shell.webContents.executeJavaScript('!!window.oyaBrowser && !!document.querySelector("#root")').catch(() => false),
  );
  await shell.webContents.executeJavaScript('window.oyaBrowser.enterBrowsing()');
  const client = await connect(await until(endpoint));
  const site = createServer((_req, res) => {
    res.setHeader('Content-Type', 'text/html');
    res.end(
      '<!doctype html><title>Production native test</title><input id=field><script>globalThis.trusted=[];document.querySelector("input").addEventListener("input",e=>trusted.push(e.isTrusted))</script>',
    );
  });
  await new Promise((resolve) => site.listen(0, '127.0.0.1', resolve));
  try {
    const url = `http://127.0.0.1:${site.address().port}/`;
    const { targetId } = await client.call('Target.createTarget', { url });
    const { sessionId } = await client.call('Target.attachToTarget', { targetId, flatten: true });
    await client.call('Runtime.evaluate', { expression: 'document.querySelector("input").focus()' }, sessionId);
    await client.call('Input.insertText', { text: 'production native 日本語' }, sessionId);
    const reply = await client.call(
      'Runtime.evaluate',
      {
        expression: '[document.querySelector("input").value,trusted.length>0&&trusted.every(Boolean)]',
        returnByValue: true,
      },
      sessionId,
    );
    assert.deepEqual(reply.result.value, ['production native 日本語', true]);
    await client.call('Page.bringToFront', {}, sessionId);
    const screenshot = await client.call('Page.captureScreenshot', { format: 'png' }, sessionId);
    assert.ok(Buffer.from(screenshot.data, 'base64').length > 100);
    await checkShield(shell);
    assert.equal((await client.call('Target.closeTarget', { targetId })).success, true);
    console.log(
      'PASS native shield: passive pointer/click/key never prompt or transfer control; explicit header asks, cancellation preserves ownership, acceptance takes control, release restores agent',
    );
  } finally {
    client.socket.terminate();
    site.closeAllConnections();
    await new Promise((resolve) => site.close(resolve));
  }
}
/** The real header receives browser-owned trusted input; no synthetic DOM click bypasses its guard. */
async function clickHeader(shell) {
  const point = await shell.webContents.executeJavaScript(`(() => {
    const button = document.querySelector('#control-action');
    const box = button.getBoundingClientRect();
    return { x: Math.round(box.x + box.width / 2), y: Math.round(box.y + box.height / 2) };
  })()`);
  shell.webContents.focus();
  shell.webContents.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, ...point });
  shell.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, ...point });
}
/** Real shield input must remain inert, and only explicit acceptance may transfer native control ownership. */
async function checkShield(shell) {
  const shield = await until(() =>
    shell.getBrowserViews().find((view) => /control-shield/.test(view.webContents.getURL())),
  );
  await until(() => shield.webContents.executeJavaScript('document.readyState === "complete"').catch(() => false));
  const state = () => shell.webContents.executeJavaScript('window.oyaBrowser.getControlState()');
  assert.equal((await state()).mode, 'agent');
  shield.webContents.focus();
  for (const point of [
    { x: 1, y: 1 },
    { x: 300, y: 200 },
  ])
    shield.webContents.sendInputEvent({ type: 'mouseMove', ...point });
  shield.webContents.sendInputEvent({ type: 'mouseDown', x: 100, y: 100, button: 'left', clickCount: 1 });
  shield.webContents.sendInputEvent({ type: 'mouseUp', x: 100, y: 100, button: 'left', clickCount: 1 });
  shield.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'A' });
  shield.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'A' });
  await new Promise((resolve) => setTimeout(resolve, 300));
  assert.equal(prompts, 0, 'passive shield events must never request handoff');
  assert.equal((await state()).mode, 'agent');
  assert.equal((await state()).interactive, false);
  assert.ok(shell.getBrowserViews().includes(shield));
  await clickHeader(shell);
  await until(() => prompts === 1);
  assert.equal((await state()).mode, 'agent', 'opening confirmation does not transfer ownership');
  answerPrompt({ response: 0 });
  await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal((await state()).mode, 'agent', 'cancellation keeps agent ownership');
  await clickHeader(shell);
  await until(() => prompts === 2);
  answerPrompt({ response: 1 });
  await until(async () => (await state()).mode === 'human');
  assert.equal((await state()).interactive, true);
  assert.ok(!shell.getBrowserViews().includes(shield));
  await clickHeader(shell);
  await until(async () => (await state()).mode === 'agent');
  assert.equal(prompts, 2);
}
run()
  .then(() => {
    clearTimeout(timeout);
    app.exit(0);
  })
  .catch((error) => {
    console.error(error);
    app.exit(1);
  });
