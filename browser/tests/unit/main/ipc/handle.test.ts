/**
 * Unit tests for IPC registration: only the shell page's main frame may call,
 * and every channel the preload exposes has a handler.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ShellIpc, shellHandlers, registerIpc } from '../../../../src/main/ipc/index.ts';
import { CALL_CHANNELS } from '../../../../src/shared/ipc.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

describe('IPC', () => {
  it('answers the shell page and refuses tabs and subframes', async () => {
    const ctx = mainCtx();
    new ShellIpc(ctx).register({ navigate: (_event: unknown, value: unknown) => value } as any);
    const call = ctx.electron.ipcMain.handlers.get('navigate');
    const shell = ctx.shell.window.webContents;
    assert.equal(await call({ sender: shell, senderFrame: shell.mainFrame }, 7), 7);
    assert.throws(() => call({ sender: {}, senderFrame: shell.mainFrame }), /Only the Oya workspace/);
    assert.throws(() => call({ sender: shell, senderFrame: {} }), /Only the Oya workspace/);
  });

  it('has a handler for every channel the preload exposes, and registers each once', () => {
    const ctx = mainCtx();
    const channels = Object.keys(shellHandlers(ctx));
    assert.deepEqual(channels.sort(), Object.values(CALL_CHANNELS).sort());
    registerIpc(ctx);
    assert.deepEqual([...ctx.electron.ipcMain.handlers.keys()].sort(), channels);
  });
});
