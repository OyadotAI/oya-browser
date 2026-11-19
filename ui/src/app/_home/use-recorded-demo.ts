/** Native playback stays in the video element; this hook only manages recoverable media errors. */
'use client';
import { useState } from 'react';

/** Recovery remains mounted after failure so retry never removes the focused button. */
type PlaybackState = 'ready' | 'failed' | 'retrying' | 'recovered';

/** Each media state has a short announcement independent of native playback controls. */
export const VIDEO_MESSAGES: Record<PlaybackState, string> = {
  ready: '',
  failed: 'The video couldn’t load. Try again or explore the interactive walkthrough below.',
  retrying: 'Reloading video…',
  recovered: 'Video reloaded. Use the controls to play it.',
};

/** Reloads the same local recording without taking playback or focus away from the visitor. */
function reloadRecording(video: HTMLVideoElement | null, announce: (state: PlaybackState) => void) {
  announce('retrying');
  video?.load();
}

/** The callback ref stores the media element; playback remains native and recovery stays local. */
export function useRecordedDemo() {
  const [video, attach] = useState<HTMLVideoElement | null>(null);
  const [state, setState] = useState<PlaybackState>('ready');
  const failed = () => setState('failed');
  const loaded = () => setState((current) => (current === 'retrying' ? 'recovered' : current));
  const retry = () => reloadRecording(video, setState);
  return { attach, state, failed, loaded, retry };
}
