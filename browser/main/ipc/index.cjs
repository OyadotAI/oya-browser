/**
 * Facade: every IPC channel the shell page can call, registered in one place
 * through the shell-only `handle` guard.
 */
const { registerHandlers } = require('./handle.cjs');
const { NAVIGATION_HANDLERS } = require('./navigation.cjs');
const { SESSION_HANDLERS } = require('./session.cjs');
const { RECORDING_HANDLERS } = require('./recording.cjs');
const { SHELL_HANDLERS } = require('./shell.cjs');
const { DEV_HANDLERS } = require('./dev.cjs');
const { ROUTINE_HANDLERS } = require('./routines.cjs');

/** Registers every channel. */
function registerIpc(handle, ctx) {
  const tables = [NAVIGATION_HANDLERS, SESSION_HANDLERS, RECORDING_HANDLERS, SHELL_HANDLERS, DEV_HANDLERS];
  for (const table of [...tables, ROUTINE_HANDLERS]) {
    registerHandlers(handle, ctx, table);
  }
}

module.exports = { registerIpc };
