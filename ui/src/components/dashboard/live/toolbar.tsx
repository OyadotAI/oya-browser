/**
 * The live view's top bar: stream status, and toggles for mouse streaming and
 * fit versus actual size.
 */
'use client';

import { Maximize2, Minimize2, MousePointer2 } from 'lucide-react';
import { frameStatus, staleSeconds } from './status';
import type { Fit } from './types';

/** What the bar shows and toggles. */
interface Props {
  /** The latest frame, or null while connecting. */
  frameSrc: string | null;
  /** Frames in the last second. */
  fps: number;
  /** Age of the latest frame. */
  frameAgeMs: number | null;
  /** Whether mouse movement is streamed. */
  hover: boolean;
  /** Fitted to the panel, or actual size. */
  fit: Fit;
  /** Flips mouse streaming. */
  onHover: () => void;
  /** Flips fit. */
  onFit: () => void;
}

/** Status dot and text, a stale-frame warning, and the two toggles. */
export default function LiveToolbar({ frameSrc, fps, frameAgeMs, hover, fit, onHover, onFit }: Props) {
  const stale = staleSeconds(frameAgeMs);
  return (
    <div className="flex h-8 items-center gap-2 border-b border-border bg-bg-card px-2 text-[11.5px] text-text-muted">
      <span className={`dot ${frameSrc ? 'dot-ok' : 'dot-dead'}`} />
      <span className="num">{frameStatus(frameSrc, fps, frameAgeMs)}</span>
      {stale !== null && <span className="text-yellow num">last frame {stale}s ago</span>}
      <span className="ml-auto" />
      <button
        className={`btn-icon h-6 w-6 ${hover ? 'text-accent' : ''}`}
        title="Stream mouse movement (uses bandwidth)"
        aria-pressed={hover}
        onClick={onHover}
      >
        <MousePointer2 className="h-3.5 w-3.5" />
      </button>
      <button className="btn-icon h-6 w-6" title={fit === 'fit' ? 'Actual size' : 'Fit to panel'} onClick={onFit}>
        {fit === 'fit' ? <Maximize2 className="h-3.5 w-3.5" /> : <Minimize2 className="h-3.5 w-3.5" />}
      </button>
    </div>
  );
}
