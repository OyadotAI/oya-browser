/**
 * Numbers and fixed text for talking CDP to a view's own debugger: the
 * analyzer's isolated world and the native dialogs it can raise.
 */

/** The protocol version Electron's debugger attaches with. */
export const CDP_VERSION = '1.3';

/**
 * A busy tab has a listener per protection step, recorder and dialog watcher on
 * its debugger; Node warns past ten, so the ceiling is raised.
 */
export const DEBUGGER_MAX_LISTENERS = 50;

/** Random bytes in an isolated world's per-document tag attribute. */
export const WORLD_ATTR_BYTES = 4;

/** Commands answerable while a dialog is held: they never reach the blocked renderer. */
export const DIALOG_SAFE_ACTIONS: ReadonlySet<string> = new Set([
  'handle_dialog',
  'screenshot',
  'list_tabs',
  'read_console',
  'read_network',
  'record',
  'workflow',
]);
