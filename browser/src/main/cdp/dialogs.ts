/** Transitional startup adapter; dialog state and decisions live in the protocol-independent service. */
import type { Debugger } from 'electron';
import { AUTO_ACCEPT } from '../../page/dialog-text.ts';
import { DialogService, type Dialog } from '../dialogs/index.ts';
export { DIALOG_HELD, DIALOG_SAFE_ACTIONS, describeDialog } from '../dialogs/index.ts';
export type { Dialog, DialogText, DialogWait, DialogAnswer, AnsweredDialog } from '../dialogs/index.ts';
/** Legacy startup's automatic policies remain unchanged until before-unload migrates. */
export const AUTO_ACCEPT_DIALOGS: ReadonlySet<string> = AUTO_ACCEPT;
/** Legacy watcher marker retained until all startup callers become native. */
export interface DialogMark {
  /** The adapter subscribes only once per debugger. */
  oyaDialogWatcher?: boolean;
}
/** Temporary transport seam, excluded from the native service. */
export type DialogDebugger = Pick<Debugger, 'on' | 'sendCommand'> & DialogMark;
/** Event fields used by the transitional adapter. */
interface OpeningParams {
  /** Native frame address reported by the old transport. */
  url?: string;
  /** Dialog kind. */
  type: string;
  /** Text displayed by the page. */
  message?: string;
  /** Prompt default is descriptive, not an implicit decision. */
  defaultPrompt?: string;
}
/** Existing startup still calls this adapter; it is not a fallback in NativeDialogs. */
export class Dialogs extends DialogService {
  /** Subscribe once to each old startup surface. */
  watch(dbg: DialogDebugger | null | undefined): void {
    if (!dbg || dbg.oyaDialogWatcher) return;
    dbg.oyaDialogWatcher = true;
    dbg.on('message', (_event, method, params) => this.onEvent(dbg, method, params));
  }
  /** Translate transport events without owning dialog decisions. */
  private onEvent(dbg: DialogDebugger, method: string, params: OpeningParams): void {
    if (method === 'Page.javascriptDialogClosed') return this.cancel(dbg);
    if (method !== 'Page.javascriptDialogOpening') return;
    this.receive(legacyDialog(dbg, params), params.url, AUTO_ACCEPT_DIALOGS.has(params.type));
  }
}
/** Keep protocol translation entirely outside the decision service. */
function legacyDialog(dbg: DialogDebugger, params: OpeningParams): Dialog {
  const reply: Dialog['reply'] = (accept, promptText) =>
    dbg.sendCommand('Page.handleJavaScriptDialog', {
      accept,
      ...(promptText === undefined ? {} : { promptText }),
    });
  return { source: dbg, reply, type: params.type, message: params.message || '', defaultPrompt: params.defaultPrompt };
}
