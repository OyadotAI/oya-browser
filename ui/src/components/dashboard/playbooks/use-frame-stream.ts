/**
 * The live view's frames while a recording is open, with a frames-per-second
 * count and when the last frame landed.
 */
import { useEffect, useRef, useState, type RefObject } from 'react';
import { subscribeFrames } from '@/lib/live-stream';
import { FPS_WINDOW_MS } from './constants';

/** The latest frame and when it arrived. */
interface Frame {
  /** The image, or null before the first frame or after the stream ends. */
  src: string | null;
  /** When it arrived (ms since the epoch). */
  at: number | null;
}

/** Where the stream reports. */
interface FrameSink {
  /** Updates the latest frame. */
  setFrame: (fn: (cur: Frame) => Frame) => void;
  /** Publishes the last window's frame count. */
  setFps: (n: number) => void;
  /** Frames seen in the current window. */
  frames: RefObject<number>;
}

/** Counts the frames seen per window into `setFps`; returns the stop. */
function countFps({ setFps, frames }: FrameSink) {
  const timer = setInterval(() => {
    setFps(frames.current);
    frames.current = 0;
  }, FPS_WINDOW_MS);
  return () => clearInterval(timer);
}

/** Subscribes to frames and counts them; returns the unsubscribe. */
function streamFrames(browserId: string, apiKey: string, sink: FrameSink) {
  const onFrame = (src: string) => {
    sink.setFrame(() => ({ src, at: Date.now() }));
    sink.frames.current++;
  };
  const stop = subscribeFrames(browserId, apiKey, onFrame, () => sink.setFrame((cur) => ({ ...cur, src: null })));
  const stopFps = countFps(sink);
  return () => (stop(), stopFps());
}

/**
 * `started`, not the recording state: the status poll replaces that object every
 * 800ms, and depending on it tore the frame stream down and rebuilt it just as
 * often — the view never got a frame, and with no frame there is nothing to map
 * a click onto.
 */
export function useFrameStream(started: boolean, browserId: string, apiKey: string) {
  const [frame, setFrame] = useState<Frame>({ src: null, at: null });
  const [fps, setFps] = useState(0);
  const frames = useRef(0);
  // The indicator reads "connecting" until a frame lands, which is the difference
  // between a view that is merely slow and one that is not there at all.
  useEffect(() => {
    if (!started || !browserId) return;
    return streamFrames(browserId, apiKey, { setFrame, setFps, frames });
  }, [started, browserId, apiKey]);
  return { frame: frame.src, fps, frameAt: frame.at };
}
