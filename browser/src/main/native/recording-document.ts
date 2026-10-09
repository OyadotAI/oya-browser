/** Capture immutable recording attribution before navigation can dispose a frame. */
import type { WebContents, WebFrameMain } from 'electron';
import { NATIVE_RECORDING } from '../../shared/native-recording.ts';
import { evaluateFrame } from './frames.ts';
import { nativeFramePath } from './frame-path.ts';
/** Attribution is captured while arming, never reconstructed from an unloading document. */
export interface NativeRecordingDocument {
  /** Random isolated-preload identity, renewed for each document. */
  readonly documentId: string;
  /** Exact native sender; a different frame cannot reuse this authorization. */
  readonly frame: WebFrameMain;
  /** Replayable owner chain at authorization time. */
  readonly frames: readonly string[];
  /** Original document URL, even after its native frame is disposed or reused. */
  readonly url: string;
}
/** Read only the isolated preload identity; missing support is never guessed from URL or frame id. */
async function documentIdentity(frame: WebFrameMain): Promise<string> {
  const token = await evaluateFrame(frame, `globalThis.${NATIVE_RECORDING.BINDING}?.documentId`);
  if (typeof token !== 'string' || !token || token.length > NATIVE_RECORDING.MAX_EPOCH_CHARS)
    throw new Error('Native recording document identity is unavailable');
  return token;
}
/** Validate both sides of path resolution so navigation cannot bind old selectors to a new document. */
export async function captureDocument(page: WebContents, frame: WebFrameMain): Promise<NativeRecordingDocument> {
  if (page.isDestroyed()) throw new Error('View is destroyed');
  const top = page.mainFrame;
  const documentId = await documentIdentity(frame);
  const url = frame.url;
  const frames = Object.freeze(await nativeFramePath(top, frame));
  if ((await documentIdentity(frame)) !== documentId || page.isDestroyed() || page.mainFrame !== top)
    throw new Error('Recording document changed while arming');
  return Object.freeze({ documentId, frame, frames, url });
}

/** Only the original document may be drained or stopped; a reused frame wrapper is not sufficient. */
export async function recordingDocumentIsCurrent(
  page: WebContents,
  document: NativeRecordingDocument,
): Promise<boolean> {
  if (page.isDestroyed() || document.frame.detached || !page.mainFrame.framesInSubtree.includes(document.frame))
    return false;
  return (await documentIdentity(document.frame)) === document.documentId;
}
