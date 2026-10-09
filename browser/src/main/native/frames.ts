/** Native isolated execution targets an exact frame, including cross-origin processes. */
import type { WebFrameMain } from 'electron';
/** Capability supplied by Oya's engine, never emulated through a debugger. */
export interface NativeAgentFrame extends Pick<WebFrameMain, 'detached'> {
  /** Execute in the fixed browser-owned agent world, not the page or preload world. */
  _executeJavaScriptInOyaWorld?: (code: string, userGesture?: boolean) => Promise<unknown>;
}
/** Fail closed on unsupported engines and stale frames; never retarget a replacement document. */
export async function evaluateFrame(frame: NativeAgentFrame, code: string): Promise<unknown> {
  if (frame.detached) throw new Error('Native agent frame is detached');
  if (typeof frame._executeJavaScriptInOyaWorld !== 'function') {
    throw new Error('This Oya engine does not support native isolated frame execution');
  }
  return frame._executeJavaScriptInOyaWorld(code, false);
}
