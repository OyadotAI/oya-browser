/**
 * The recording player's state: the frame list, the current frame as an
 * object URL, and timed playback. Frames are fetched individually and cached
 * by the browser.
 */
import { useEffect, useMemo, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from 'react';
import { authHeaders } from '@/lib/api';
import { MIN_FRAMES_TO_PLAY } from './constants';
import { frameGap } from './format';
import { fetchFrame, fetchFrames, whileMounted } from './requests';
import type { Frame } from './types';

/** A state setter, as the loaders see it. */
type Setter<T> = (value: T) => void;

/** Everything the player shows and the controls change. */
export function usePlayer(sessionId: string, apiKey: string) {
  const headers = useMemo(() => authHeaders(apiKey), [apiKey]);
  const { frames, error } = useFrames(sessionId, headers);
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(true);
  const src = useFrameImage(sessionId, headers, frames, index);
  usePlayback(playing, frames, index, setIndex);
  const seek = (i: number) => (setPlaying(false), setIndex(i));
  return { frames, error, index, playing, src, toggle: () => setPlaying((p) => !p), seek };
}

/** The recording's frame list, or why it could not be loaded. */
function useFrames(sessionId: string, headers: HeadersInit) {
  const [frames, setFrames] = useState<Frame[]>([]);
  const [error, setError] = useState('');
  useEffect(() => loadFrames(sessionId, headers, setFrames, setError), [sessionId, headers]);
  return { frames, error };
}

/** The current frame as an object URL; the previous one is revoked as it is replaced. */
function useFrameImage(sessionId: string, headers: HeadersInit, frames: Frame[], index: number) {
  const [src, setSrc] = useState('');
  const objectUrl = useRef<string | null>(null);
  useEffect(() => {
    if (!frames.length) return;
    return loadFrame(sessionId, frames[index]?.i ?? 0, headers, (blob) => show(objectUrl, blob, setSrc));
  }, [frames, index, sessionId, headers]);
  // Revoke the last object URL on unmount, not on every frame change.
  useEffect(() => () => revoke(objectUrl), []);
  return src;
}

/** Fetches the frame list; the returned cleanup drops a late answer. */
function loadFrames(sessionId: string, headers: HeadersInit, setFrames: Setter<Frame[]>, setError: Setter<string>) {
  return whileMounted((live) => {
    fetchFrames(sessionId, headers).then(
      (list) => live() && setFrames(list),
      () => live() && setError('Could not load the recording'),
    );
  });
}

/** Fetches frame `i` and hands it to `onBlob`; failures are ignored and the old frame stays. */
function loadFrame(sessionId: string, i: number, headers: HeadersInit, onBlob: (blob: Blob) => void) {
  return whileMounted((live) => {
    fetchFrame(sessionId, i, headers)
      .then((blob) => live() && onBlob(blob))
      .catch(() => {});
  });
}

/** Swaps in a new frame blob, revoking the one it replaces. */
function show(objectUrl: MutableRefObject<string | null>, blob: Blob, setSrc: (src: string) => void) {
  revoke(objectUrl);
  objectUrl.current = URL.createObjectURL(blob);
  setSrc(objectUrl.current);
}

/** Frees the object URL held in `objectUrl`, if any. */
function revoke(objectUrl: MutableRefObject<string | null>) {
  if (objectUrl.current) URL.revokeObjectURL(objectUrl.current);
}

/** Advances a playing recording by each frame's real gap, looping at the end. */
function usePlayback(playing: boolean, frames: Frame[], index: number, setIndex: Dispatch<SetStateAction<number>>) {
  useEffect(() => {
    if (!playing || frames.length < MIN_FRAMES_TO_PLAY) return;
    const t = setTimeout(() => setIndex((i) => (i + 1 < frames.length ? i + 1 : 0)), frameGap(frames, index));
    return () => clearTimeout(t);
  }, [playing, index, frames, setIndex]);
}
