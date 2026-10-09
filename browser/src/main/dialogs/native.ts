/** Native dialog ownership for agent-controlled pages; no debugging fallback exists. */
import {
  watchNativeDialogs,
  type NativeDialogPage,
  type NativeDialogInfo,
  type NativeDialogReply,
} from '../native/index.ts';
import { DialogService } from './service.ts';
import type { Dialog } from './types.ts';
/** Preserve native callback identity when shaping a dialog for the shared queue. */
function nativeDialog(info: NativeDialogInfo, reply: NativeDialogReply): Dialog {
  return {
    source: reply,
    reply,
    type: info.dialogType,
    message: info.messageText,
    defaultPrompt: info.defaultPromptText,
  };
}
/** Browser callbacks feed the same command-facing decision queue as the rest of the app. */
export class NativeDialogs extends DialogService {
  /** Weak keys do not keep closed web contents alive. */
  private readonly watched = new WeakMap<NativeDialogPage, () => void>();
  /** Registration is explicit and requires the patched native engine. */
  watch(page: NativeDialogPage): void {
    if (this.watched.has(page)) return;
    const stop = watchNativeDialogs(
      page,
      (info, reply) => this.receive(nativeDialog(info, reply), info.frame.url),
      (reply) => this.cancel(reply),
    );
    this.watched.set(page, stop);
  }
  /** Restore human dialog ownership only after outstanding decisions have settled. */
  unwatch(page: NativeDialogPage): void {
    this.watched.get(page)?.();
    this.watched.delete(page);
  }
}
