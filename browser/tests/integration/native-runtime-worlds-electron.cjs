/** Engine-owned isolated worlds must never share globals, handles or preload capabilities across agents. */
const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { once } = require('node:events');
const { mkdtempSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const profile = mkdtempSync(join(tmpdir(), 'oya-native-worlds-'));
const DEADLINE_MS = 30000;
app.setPath('userData', profile);
const deadline = setTimeout(() => {
  console.error('Native isolated-world test timed out');
  app.exit(1);
}, DEADLINE_MS);
/** Native operation failures must not masquerade as successful undefined results. */
async function perform(frame, owner, context, operation, params = {}) {
  const reply = await frame._runOyaRuntime(owner, context, operation, params);
  if (reply.error) throw Error(reply.error);
  return reply;
}
/** Every operation carries the exact native context token and opaque world key. */
function client(frame, owner, context, world) {
  return {
    run: (source) => perform(frame, owner, context, 'evaluate', { world, source, byValue: true }),
    call: (operation, params = {}) => perform(frame, owner, context, operation, { ...params, world }),
  };
}
/** Different agent namespaces, page globals, handles and navigation all remain independent. */
async function check(window, url) {
  await window.loadURL(url);
  const frame = window.webContents.mainFrame;
  await frame._executeJavaScriptInOyaWorld('globalThis.internalRecorderSecret = 93');
  const main = (await perform(frame, 'alpha', '', 'context')).context;
  for (let i = 0; i < 70; i++) {
    const temporary = client(frame, 'ordinary-' + i, main, '');
    assert.equal((await temporary.run('42')).result.value, 42);
    await temporary.call('close');
  }
  const create = (owner, world) => perform(frame, owner, main, 'isolatedContext', { world });
  const a = (await create('alpha', 'shared-name')).context;
  const b = (await create('beta', 'shared-name')).context;
  const c = (await create('alpha', 'other-name')).context;
  assert.equal((await create('alpha', 'shared-name')).context, a, 'same owned world is stable');
  assert.equal(new Set([main, a, b, c]).size, 4, 'worlds have distinct native identities');
  const first = client(frame, 'alpha', a, 'shared-name');
  const second = client(frame, 'beta', b, 'shared-name');
  const sibling = client(frame, 'alpha', c, 'other-name');
  assert.equal((await first.run('globalThis.secret=42')).result.value, 42);
  assert.equal((await second.run('typeof secret')).result.value, 'undefined');
  assert.equal((await sibling.run('typeof secret')).result.value, 'undefined');
  assert.equal((await client(frame, 'alpha', main, '').run('typeof secret')).result.value, 'undefined');
  assert.equal((await first.run('typeof pageSecret')).result.value, 'undefined');
  assert.equal((await first.run('typeof internalRecorderSecret')).result.value, 'undefined');
  assert.equal((await first.call('evaluate', { source: 'Promise.resolve(42)', await: true })).result.value, 42);
  assert.ok((await first.run('throw Error("isolated failure")')).exception);
  await assert.rejects(create('alpha', ''), /invalid/);
  await assert.rejects(create('alpha', 42), /Invalid/);
  await assert.rejects(create('alpha', 'x'.repeat(129)), /Invalid/);
  assert.equal(
    (await first.run('typeof process + ":" + typeof require + ":" + typeof __oyaNativeRecording')).result.value,
    'undefined:undefined:undefined',
  );
  await first.run('document.querySelector("input").value="shared DOM"');
  assert.equal((await second.run('document.querySelector("input").value')).result.value, 'shared DOM');
  const object = await first.call('evaluate', { source: '({value:42})' });
  const handle = object.result.objectId;
  assert.ok(handle);
  assert.equal(
    (await first.call('invoke', { receiver: handle, source: 'function(){return this.value + secret}' })).result.value,
    84,
  );
  await assert.rejects(second.call('inspect', { object: handle }), /foreign/);
  await assert.rejects(sibling.call('inspect', { object: handle }), /foreign/);
  await assert.rejects(perform(frame, 'beta', a, 'evaluate', { world: 'shared-name', source: '1' }), /replaced/);
  await assert.rejects(perform(frame, 'beta', a, 'evaluate', { world: 'missing', source: '1' }), /foreign/);
  assert.ok((await first.call('inspect', { object: handle })).properties.some((p) => p.name === 'value'));
  assert.ok(
    (await first.run('frames[0].document.body')).exception,
    'isolated world cannot bypass cross-origin frame security',
  );
  const child = frame.frames[0];
  assert.ok(child, 'fixture owns a real child frame');
  const childMain = (await perform(child, 'alpha', '', 'context')).context;
  await assert.rejects(perform(child, 'alpha', main, 'isolatedContext', { world: 'shared-name' }), /replaced/);
  const childWorld = (await perform(child, 'alpha', childMain, 'isolatedContext', { world: 'shared-name' })).context;
  assert.notEqual(childWorld, a);
  const nested = client(child, 'alpha', childWorld, 'shared-name');
  assert.equal((await nested.run('typeof secret')).result.value, 'undefined');
  await assert.rejects(nested.call('inspect', { object: handle }), /foreign/);
  await first.call('close');
  await assert.rejects(first.run('1'), /replaced/);
  await assert.rejects(create('alpha', 'shared-name'), /replaced/);
  assert.equal((await second.run('6*7')).result.value, 42, 'another agent survives close');
  await window.loadURL(url + '?replacement');
  await assert.rejects(second.run('1'), /replaced|foreign|disposed/);
  const replacement = (await perform(window.webContents.mainFrame, 'beta', '', 'context')).context;
  assert.notEqual(replacement, main);
  const fresh = window.webContents.mainFrame;
  const freshWorld = (await perform(fresh, 'alpha', replacement, 'isolatedContext', { world: 'shared-name' })).context;
  assert.notEqual(freshWorld, a);
  assert.equal(
    (await client(fresh, 'alpha', freshWorld, 'shared-name').run('typeof secret')).result.value,
    'undefined',
  );
}
/** Only disposable loopback pages and the explicit patched Oya engine participate. */
async function run() {
  await app.whenReady();
  const childSite = createServer((_req, res) => res.end('<!doctype html><input>'));
  childSite.listen(0, '127.0.0.1');
  await once(childSite, 'listening');
  const childUrl = `http://127.0.0.1:${childSite.address().port}/`;
  const site = createServer((_req, res) =>
    res.end(`<!doctype html><script>window.pageSecret=17</script><input><iframe src="${childUrl}"></iframe>`),
  );
  site.listen(0, '127.0.0.1');
  await once(site, 'listening');
  const window = new BrowserWindow({
    show: false,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('Engine debugger is forbidden');
    },
  });
  try {
    await check(window, `http://127.0.0.1:${site.address().port}/`);
    console.log('PASS native isolated-world ownership, globals, DOM, handles, close and navigation');
  } finally {
    window.destroy();
    await new Promise((resolve) => site.close(resolve));
    await new Promise((resolve) => childSite.close(resolve));
  }
}
run()
  .then(() => {
    clearTimeout(deadline);
    rmSync(profile, { recursive: true, force: true });
    app.exit(0);
  })
  .catch((error) => {
    console.error(error);
    clearTimeout(deadline);
    app.exit(1);
  });
