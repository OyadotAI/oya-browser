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

import { NATIVE_DIALOG } from '../../shared/native-dialog.ts';
/** Browser-owned sheets are fixed-size, never styled by the requesting page. */
export const DIALOG_WINDOW_OPTIONS = {
  width: NATIVE_DIALOG.WIDTH,
  height: NATIVE_DIALOG.HEIGHT,
  show: false,
  title: 'Oya · Page dialog',
  resizable: false,
  minimizable: false,
  maximizable: false,
  backgroundColor: '#10120f',
};
/** A sheet's own content cannot create another dialog, webview or privileged renderer. */
export const DIALOG_WEB_PREFERENCES = {
  sandbox: true,
  contextIsolation: true,
  nodeIntegration: false,
  webviewTag: false,
  disableDialogs: true,
};
