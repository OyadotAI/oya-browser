/**
 * Recordings at rest: every frame and manifest is sealed before it touches the
 * spool or the bucket, and opened on read. A recording is a picture of
 * someone's browser, as sensitive as the cookies in it.
 *
 * Scopes name the recording (and the frame), so a frame copied into another
 * recording, or another frame's place, fails to open. The owner travels inside
 * the sealed manifest, where the read path checks it. Recordings written
 * before sealing are plain JPEG and JSON and still read: a sealed buffer
 * starts with a version byte neither format can start with.
 */
import { isSealed, open, openBytes, seal, sealBytes } from '../../../platform/secrets.ts';

/** The scope a recording's manifest is sealed under. */
const manifestScope = (id: string) => `recording:${id}`;
/** The scope one frame is sealed under. */
const frameScope = (id: string, index: number) => `recording:${id}:${index}`;

/** A frame's JPEG, sealed for its recording and position. */
export const sealFrame = (id: string, index: number, jpeg: Buffer) => sealBytes(frameScope(id, index), jpeg);

/** A frame's JPEG from what was stored: opened when sealed, as-is when written before sealing. */
export const openFrame = (id: string, index: number, stored: Buffer) =>
  isSealed(stored) ? openBytes(frameScope(id, index), stored) : stored;

/** A manifest, sealed for its recording. */
export const sealManifest = (manifest) => seal(manifestScope(manifest.sessionId), manifest);

/** A manifest from what was stored: opened when sealed, parsed as-is when written before sealing. */
export const openManifest = (id: string, stored: Buffer) =>
  isSealed(stored) ? open(manifestScope(id), stored) : JSON.parse(stored.toString('utf8'));
