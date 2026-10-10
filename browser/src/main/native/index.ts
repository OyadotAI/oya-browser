/** Native browser operations shared by internal callers and external protocol adapters. */
export { evaluatePage, capturePage, screenshotOptions, type NativePage } from './page.ts';
export { World, type WorldDeps, type EnsureOptions, type EvalOptions } from './world.ts';
export { evaluateFrame, type NativeAgentFrame } from './frames.ts';
export { watchNativeDialogs, type NativeDialogInfo, type NativeDialogReply, type NativeDialogPage } from './dialogs.ts';
export { NativeRecordingInbox, type NativeRecordingMessage } from './recording.ts';
export { nativeFramePath } from './frame-path.ts';
export type { NativeRecordingDocument } from './recording-document.ts';
export { NativeDocumentRecorder } from './document-recorder.ts';
export { NativeInspection } from './inspection.ts';
export { watchNativeLog, type NativeEventSink } from './log-stream.ts';
export { NativeFrameTree } from './frame-tree.ts';
export { NativeDeviceMetrics, metricParameters } from './device-metrics.ts';
export { watchNativePage, nativePageCommand } from './page-events.ts';
export { insertNativeText } from './text-input.ts';
export { NativeRuntime } from './runtime.ts';
export { NativeNavigationHistory } from './navigation-history.ts';
export { recordingDocumentIsCurrent } from './recording-document.ts';

export { validatePointer } from './pointer-input.ts';
export { validateKey } from './keyboard-input.ts';
export { dispatchNativeInput } from './input-commands.ts';
