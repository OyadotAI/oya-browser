/**
 * The open browser's live frames, and how many arrive per second.
 */
import { useEffect, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from 'react';
import { subscribeFrames } from '@/lib/live-stream';
import { FPS_WINDOW_MS } from './constants';

/** The latest frame and when it came. */
interface LastFrame {
  /** Frame image as a data URL; null while the stream is down. */
  frame: string | null;
  /** When it arrived (ms since epoch); kept when the stream drops. */
  at: number | null;
}

/** Setter for the latest frame. */
type SetLast = Dispatch<SetStateAction<LastFrame>>;

/** Records a frame and counts it toward the fps readout. */
const countFrame = (frames: RefObject<number>, setLast: SetLast) => (frame: string) => {
  frames.current++;
  setLast({ frame, at: Date.now() });
};

/** The stream dropped: no frame, but the last arrival time stays. */
const dropFrame = (l: LastFrame): LastFrame => ({ ...l, frame: null });

/** Reads and resets the frame count for one fps window. */
function takeCount(frames: RefObject<number>): number {
  const n = frames.current;
  frames.current = 0;
  return n;
}

/** Subscribes to the browser's frames for as long as it is open. */
function useFrameStream(browserId: string, apiKey: string, frames: RefObject<number>) {
  const [last, setLast] = useState<LastFrame>({ frame: null, at: null });
  useEffect(
    () => subscribeFrames(browserId, apiKey, countFrame(frames, setLast), () => setLast(dropFrame)),
    [browserId, apiKey, frames],
  );
  return last;
}

/** Frames counted over each FPS_WINDOW_MS; restarts when the browser changes. */
function useFps(frames: RefObject<number>, browserId: string, apiKey: string) {
  const [fps, setFps] = useState(0);
  useEffect(() => {
    const fpsTimer = setInterval(() => setFps(takeCount(frames)), FPS_WINDOW_MS);
    return () => clearInterval(fpsTimer);
  }, [frames, browserId, apiKey]);
  return fps;
}

/** Latest frame (null while the stream is down), frames per second, and when the last one came. */
export function useLiveFrames(browserId: string, apiKey: string) {
  const frames = useRef(0);
  const { frame, at } = useFrameStream(browserId, apiKey, frames);
  const fps = useFps(frames, browserId, apiKey);
  return { frame, fps, frameAt: at };
}
