/**
 * Session recording.
 *
 * Frames come from CDP Page.startScreencast on a second connection to the same
 * browser — nothing is injected into the page, and the client's own wire is
 * untouched, so a client using screencast itself is unaffected.
 *
 * Recording is opt-in per session (?record=1) because storage scales with
 * fleet size: at 1k-5k sessions, recording everything by default would be the
 * largest thing this control plane writes.
 *
 * This file is the facade: live recording is in recording-live.ts, stored
 * recordings in recording-library.ts.
 */
export { isRecording, start, stop } from './recording-live.ts';
export { list, manifest, frame, remove, maintain } from './recording-library.ts';
