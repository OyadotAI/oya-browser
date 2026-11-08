/** Native isolated execution targets an exact frame, cross-process frames included, on stock Electron and the patched engine. */
import type { WebContents, WebFrameMain } from 'electron';
import { ANALYZER_WORLD_ID } from './constants.ts';
import { runInWorld } from './stock-world.ts';
/** Capability supplied by Oya's engine, never emulated through a debugger. */
export interface NativeAgentFrame extends Pick<WebFrameMain, 'detached'> {
  /** Execute in the fixed browser-owned agent world, not the page or preload world. */
  _executeJavaScriptInOyaWorld?: (code: string, userGesture?: boolean) => Promise<unknown>;
}
/** The page owning a frame, as stock Electron's `webContents.fromFrame` answers it. */
export type FramePage = (
  frame: NativeAgentFrame,
) => Pick<WebContents, 'mainFrame' | 'executeJavaScriptInIsolatedWorld'> | undefined;
/** Supplied by the composition root on every engine; only used where the patched API is missing. */
let pageOf: FramePage = () => undefined;
/** Stock Electron (desktop) can run the agent world in main frames through the owning page. */
export function useFramePages(lookup: FramePage): void {
  pageOf = lookup;
}
/** Fail closed on unsupported frames and stale frames; never retarget a replacement document. */
export async function evaluateFrame(frame: NativeAgentFrame, code: string): Promise<unknown> {
  if (frame.detached) throw new Error('Native agent frame is detached');
  if (typeof frame._executeJavaScriptInOyaWorld !== 'function') return evaluateStock(frame, code);
  return frame._executeJavaScriptInOyaWorld(code, false);
}
/**
 * Same agent world through stock Electron. A subframe, cross-origin ones included, is
 * reached through the tab preload every frame loads (nodeIntegrationInSubFrames), which
 * runs the code in the isolated world in that frame's own process: the page never sees it.
 */
function evaluateStock(frame: NativeAgentFrame, code: string): Promise<unknown> {
  const page = pageOf(frame);
  if (!page) throw new Error('This Oya engine does not support native isolated frame execution');
  return runInWorld(page as WebContents, ANALYZER_WORLD_ID, code, { frame: frame as WebFrameMain });
}
