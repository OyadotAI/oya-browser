/** Convert engine metadata to text-only UI data without exposing credentials or callbacks in URLs. */
import type { NativeDialogInfo } from '../native/index.ts';
import type { DialogPresentation } from '../../shared/native-dialog.ts';
/** Opaque and local documents are labelled without exposing file paths or URL query strings. */
function dialogOrigin(address: string): string {
  try {
    const url = new URL(address);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.origin : 'This page';
  } catch {
    return 'This page';
  }
}
/** Browser-owned positive and safe-default labels. */
function labels(info: NativeDialogInfo) {
  if (info.dialogType !== 'beforeunload') return { title: 'This page asks', accept: 'OK', cancel: 'Cancel' };
  return { title: 'Leave this page?', accept: info.isReload ? 'Reload page' : 'Leave page', cancel: 'Stay on page' };
}
/** Unload wording is browser-owned because page-provided text is not a reliable safety warning. */
export function dialogPresentation(info: NativeDialogInfo): DialogPresentation {
  return {
    ...labels(info),
    origin: dialogOrigin(info.frame.url),
    message: info.dialogType === 'beforeunload' ? 'Changes you made may not be saved.' : info.messageText,
    prompt: info.dialogType === 'prompt',
    value: info.defaultPromptText,
  };
}
