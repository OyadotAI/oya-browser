/** Subscribe to native renderer dialogs without changing page JavaScript or opening a debugger. */
import type { WebContents, WebFrameMain } from 'electron';
/** Reply belongs to exactly one native dialog; the engine ignores repeated or stale answers. */
export type NativeDialogReply = (accept: boolean, text?: string) => void;
/** Native dialog metadata, including its actual originating frame. */
export interface NativeDialogInfo {
  /** A native alert, confirmation, prompt or before-unload decision. */
  dialogType: string;
  /** Untrusted page-provided text, to display as text only. */
  messageText: string;
  /** Initial prompt value; never an implicit answer. */
  defaultPromptText: string;
  /** Present for unload decisions so presenters can distinguish reload from leaving. */
  isReload?: boolean;
  /** Native originating frame, not a URL-based lookup. */
  frame: Pick<WebFrameMain, 'url'>;
}
/** Caller retains the decision; subscribing never automatically accepts a dialog. */
export type NativeDialogHandler = (info: NativeDialogInfo, reply: NativeDialogReply) => void;
/** The patched engine's browser-process-only capability. */
export interface NativeDialogPage extends Pick<WebContents, 'isDestroyed'> {
  /** Engine cancellation identifies the exact callback that is no longer usable. */
  on(event: '-oya-dialog-cancelled', listener: (reply: NativeDialogReply) => void): unknown;
  /** Remove only this subscription when it is disposed. */
  off(event: '-oya-dialog-cancelled', listener: (reply: NativeDialogReply) => void): unknown;
  /** Explicit marker prevents older engines silently bypassing unload decisions. */
  _oyaBeforeUnloadDialogs?: boolean;
  /** Null restores ordinary human dialogs, but only when no agent dialog is pending. */
  _setOyaDialogHandler?: (handler: NativeDialogHandler | null) => void;
}
/** Refuse unsupported engines instead of falling back to debugging or page-level dialog shims. */
export function watchNativeDialogs(
  page: NativeDialogPage,
  handler: NativeDialogHandler,
  cancelled: (reply: NativeDialogReply) => void,
): () => void {
  register(page, handler);
  page.on('-oya-dialog-cancelled', cancelled);
  return unsubscribe(page, cancelled);
}
/** Idempotent disposal cannot clear a later subscriber; a pending dialog leaves this subscription intact. */
function unsubscribe(page: NativeDialogPage, cancelled: (reply: NativeDialogReply) => void): () => void {
  let disposed = false;
  return () => {
    if (disposed) return;
    if (!page.isDestroyed()) page._setOyaDialogHandler!(null);
    disposed = true;
    page.off('-oya-dialog-cancelled', cancelled);
  };
}
/** Check the engine capability before subscribing to any events. */
function register(page: NativeDialogPage, handler: NativeDialogHandler): void {
  if (page.isDestroyed()) throw new Error('View is destroyed');
  if (!page._setOyaDialogHandler) throw new Error('This Oya engine does not support native dialog handling');
  if (page._oyaBeforeUnloadDialogs !== true)
    throw new Error('This Oya engine does not support native before-unload dialogs');
  page._setOyaDialogHandler(handler);
}
