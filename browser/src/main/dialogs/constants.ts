/** Dialog coordination signals and commands that do not enter a blocked renderer. */
export const DIALOG_HELD = Symbol('dialog held');
/** Safe commands remain available so the driver can inspect and answer a held dialog. */
export const DIALOG_SAFE_ACTIONS: ReadonlySet<string> = new Set([
  'handle_dialog',
  'screenshot',
  'list_tabs',
  'read_console',
  'read_network',
  'record',
  'workflow',
]);
