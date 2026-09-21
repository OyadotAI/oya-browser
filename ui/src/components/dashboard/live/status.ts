/**
 * The live view's status line: how fresh the stream is, in words.
 */
import { LIVE_FRAME_MS, MS_PER_SECOND, STALE_FRAME_MS } from './constants';

/** "connecting", "N fps", "live" or "idle". An idle page paints ~1 frame/s from the screenshot fill; that is "live", not "0 fps". */
export function frameStatus(frameSrc: string | null, fps: number, frameAgeMs: number | null): string {
  if (!frameSrc) return 'connecting';
  if (fps > 1) return `${fps} fps`;
  return frameAgeMs !== null && frameAgeMs < LIVE_FRAME_MS ? 'live' : 'idle';
}

/** Seconds since the last frame when it is old enough to call out, else null. */
export function staleSeconds(frameAgeMs: number | null): number | null {
  if (frameAgeMs === null || frameAgeMs <= STALE_FRAME_MS) return null;
  return Math.round(frameAgeMs / MS_PER_SECOND);
}
