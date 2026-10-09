/** Native frame membership and lifetime fence request continuations without URL matching or polling. */
import type { WebContents, WebFrameMain } from 'electron';
import type { RequestDetails } from './types.ts';
/** Snapshot the native renderer identity rather than retaining a lazy request getter. */
export interface FrameLease {
  /** Exact initiating frame wrapper. */
  frame: WebFrameMain;
  /** Native renderer token changes when its host is replaced. */
  token: string;
  /** Native process identity used by committed-navigation events. */
  process: number;
  /** Exact frame route within that process. */
  routing: number;
}
/** Missing/foreign frames are never relabelled as requests from the active main page. */
export function ownedFrame(contents: WebContents, frame: WebFrameMain | null | undefined): frame is WebFrameMain {
  return !!frame && !frame.detached && !contents.isDestroyed() && contents.mainFrame.framesInSubtree.includes(frame);
}
/** Capture immutable renderer identity at the original native pause. */
export function leaseRequest(details: RequestDetails): FrameLease {
  const frame = details.frame,
    contents = details.webContents;
  if (!contents || !ownedFrame(contents, frame)) throw Error('Native request frame is unavailable');
  return { frame, token: frame.frameToken, process: frame.processId, routing: frame.routingId };
}
/** Destruction, graph removal and host replacement all revoke the continuation. */
export function liveLease(contents: WebContents, lease: FrameLease): boolean {
  try {
    return ownedFrame(contents, lease.frame) && lease.frame.frameToken === lease.token;
  } catch {
    return false;
  }
}
