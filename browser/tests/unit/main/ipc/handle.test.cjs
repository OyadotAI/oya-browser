/**
 * Unit tests for IPC registration: only the shell page's main frame may call,
 * and every table's channels reach their handler with the context.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { createHandle } = require('../../../../main/ipc/handle.cjs');
const { registerIpc } = require('../../../../main/ipc/index.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('IPC', () => {
  it('answers the shell page and refuses tabs and subframes', async () => {
    const ctx = mainCtx();
    createHandle(ctx)('echo', (_event, value) => value);
    const call = ctx.electron.ipcMain.handlers.get('echo');
    const shell = ctx.shell.window.webContents;
    assert.equal(await call({ sender: shell, senderFrame: shell.mainFrame }, 7), 7);
    assert.throws(() => call({ sender: {}, senderFrame: shell.mainFrame }), /Only the Oya workspace/);
    assert.throws(() => call({ sender: shell, senderFrame: {} }), /Only the Oya workspace/);
  });

  it('registers every channel the preload exposes', () => {
    const ctx = mainCtx();
    const channels = [];
    registerIpc((channel) => channels.push(channel), ctx);
    const preload = require('node:fs').readFileSync(require.resolve('../../../../preload.js'), 'utf8');
    const invoked = [...preload.matchAll(/invoke\('([\w-]+)'/g)].map((m) => m[1]);
    const updater = ['get-version', 'get-update-status', 'check-for-updates', 'install-update'];
    for (const channel of invoked) if (!updater.includes(channel)) assert.ok(channels.includes(channel), channel);
    assert.equal(new Set(channels).size, channels.length, 'a channel registered twice');
  });
});
