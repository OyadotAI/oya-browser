/**
 * Session recording.
 *
 * Oya fleet attachments sample an exact protected tab through authorized native
 * screenshot commands. External providers retain their separate screencast
 * connection. Nothing is injected and the client's compatibility wire is untouched.
 * Native sampling stops on ownership/control loss; a sealed captureError explains
 * interruption. Frame writes settle before the sealed manifest is archived.
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
