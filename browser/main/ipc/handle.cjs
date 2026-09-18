/**
 * Every IPC channel is the shell's. Tabs and the control shield have no preload,
 * so a call from anywhere else is not ours to answer.
 */

/** Throws unless the call came from the shell page's main frame. */
function requireShell(ctx, event) {
  const shell = ctx.shell.window;
  if (event.sender !== shell?.webContents || event.senderFrame !== shell.webContents.mainFrame) {
    throw new Error('Only the Oya workspace can use this command');
  }
}

/** `handle(channel, fn)`: ipcMain.handle, guarded by requireShell. */
function createHandle(ctx) {
  return (channel, fn) =>
    ctx.electron.ipcMain.handle(channel, (event, ...args) => {
      requireShell(ctx, event);
      return fn(event, ...args);
    });
}

/** Registers a table of channel → `(ctx, event, ...args)` handlers. */
function registerHandlers(handle, ctx, table) {
  for (const [channel, fn] of Object.entries(table)) handle(channel, (event, ...args) => fn(ctx, event, ...args));
}

module.exports = { createHandle, registerHandlers, requireShell };
