/** Browser-owned dialog coordination, independent of protocol compatibility. */
export { DialogService, describeDialog } from './service.ts';
export { NativeDialogs } from './native.ts';
export { DIALOG_HELD, DIALOG_SAFE_ACTIONS } from './constants.ts';
export type { Dialog, DialogText, DialogWait, DialogAnswer, AnsweredDialog } from './types.ts';
export { DesktopDialogs } from './desktop.ts';
export { dialogPresenter } from './windows.ts';
