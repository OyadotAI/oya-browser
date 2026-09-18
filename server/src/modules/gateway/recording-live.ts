/**
 * Recordings in progress on this replica: starting the screencast, spooling
 * its frames to disk, and writing the manifest when it stops.
 */
import { control } from '../control/service.ts';
import { archiveRecording, sharedRecordings } from '../control/recording-storage.ts';
import { mkdir, writeFile } from 'fs/promises';
import { join } from 'path';
import { metrics } from '../../platform/metrics.ts';
import { dataPath } from '../../platform/paths.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { openPage } from './cdp-page.ts';
import {
  FRAME_NAME_DIGITS,
  PRIVATE_DIR_MODE,
  RECORD_EVERY_NTH,
  RECORD_MAX_BYTES,
  RECORD_MAX_FRAMES,
  RECORD_QUALITY,
  SCREENCAST_MAX_HEIGHT,
  SCREENCAST_MAX_WIDTH,
} from './constants.ts';

/** Where recordings are spooled. */
export const DIR = dataPath('recordings');

/** sessionId -> { conn, sessionId, dir, frames, bytes, startedAt, provider, owner, profile } */
export const active = new Map();

/** What Chrome is asked to stream. */
const SCREENCAST_OPTIONS = {
  format: 'jpeg',
  quality: RECORD_QUALITY,
  maxWidth: SCREENCAST_MAX_WIDTH,
  maxHeight: SCREENCAST_MAX_HEIGHT,
  everyNthFrame: RECORD_EVERY_NTH,
};

/** A frame's file name within its recording's directory. */
export const frameFile = (index) => `${String(index).padStart(FRAME_NAME_DIGITS, '0')}.jpg`;

/** Whether a recording has reached its frame or byte cap. */
const isFull = (state) => state.frames.length >= RECORD_MAX_FRAMES || state.bytes >= RECORD_MAX_BYTES;

/** Whether this replica is recording the session now. */
export function isRecording(sessionId) {
  return active.has(sessionId);
}

/**
 * Start recording a session's first page to the local spool. False if already
 * recording or there is no page; refused under a visual-redaction policy, and
 * across replicas without shared recording storage.
 */
export async function start(session) {
  if (active.has(session.id)) return false;
  await assertRecordable(session);
  const attached = await openPage(session.upstreamUrl);
  if (!attached) return false;
  await beginScreencast(await track(session, attached));
  metrics.recordings.inc({ event: 'start' });
  return true;
}

/** Asks Chrome to start streaming the page. */
async function beginScreencast(state) {
  await state.conn.send('Page.enable', {}, state.sessionId).catch(() => {});
  await state.conn.send('Page.startScreencast', SCREENCAST_OPTIONS, state.sessionId);
}

/** Refuses (422) a session whose policy forbids recording, or a replica that cannot share recordings. */
async function assertRecordable(session) {
  const resource = await control().store.get('session', session.attachedTo || session.id);
  const project = resource && (await control().store.get('project', resource.project));
  if ([...(resource?.policies || []), project?.settings.policy || {}].some((p) => p.redactRecording))
    throw new HttpError(Status.UNPROCESSABLE, 'Recording is disabled by the visual-redaction policy');
  if (process.env.OYA_INSTANCE_URL && !sharedRecordings())
    throw new HttpError(Status.UNPROCESSABLE, 'Distributed recording requires OYA_RECORDING_BUCKET');
}

/** Creates the spool directory and starts collecting the page's screencast frames. */
async function track(session, { conn, sessionId }) {
  const dir = join(DIR, session.id);
  await mkdir(dir, { recursive: true, mode: PRIVATE_DIR_MODE });
  const state = { conn, sessionId, dir, frames: [], bytes: 0, startedAt: Date.now(), ...ownership(session) };
  active.set(session.id, state);
  conn.on('Page.screencastFrame', async (params) => onFrame(state, params));
  return state;
}

/** Who the recording belongs to and where the browser came from. */
function ownership(session) {
  return { provider: session.provider, owner: session.owner, profile: session.profile };
}

/** One screencast frame: acknowledge it, then spool it unless the recording is full. */
function onFrame(state, params) {
  // Ack first: Chrome stops sending until the frame is acknowledged, and a
  // slow disk must not stall the recorded browser.
  state.conn.send('Page.screencastFrameAck', { sessionId: params.sessionId }, state.sessionId).catch(() => {});
  if (isFull(state)) return;
  const buf = Buffer.from(params.data, 'base64');
  const index = state.frames.length;
  state.frames.push(frameEntry(state, index, params, buf));
  state.bytes += buf.length;
  metrics.recordedFrames.inc({});
  writeFile(join(state.dir, frameFile(index)), buf).catch(() => {});
}

/** The manifest's record of one frame: index, offset, viewport and size. */
function frameEntry(state, index, params, buf) {
  return {
    i: index,
    t: Date.now() - state.startedAt,
    meta: params.metadata ? { w: params.metadata.deviceWidth, h: params.metadata.deviceHeight } : null,
    bytes: buf.length,
  };
}

/** Stop recording, write the manifest, and archive it when shared storage is configured. False if not recording. */
export async function stop(sessionId) {
  const state = active.get(sessionId);
  if (!state) return false;
  active.delete(sessionId);
  await endScreencast(state);
  await saveManifest(manifestOf(sessionId, state), state.dir);
  metrics.recordings.inc({ event: 'stop' });
  return true;
}

/** Stops the screencast and closes the recording connection. */
async function endScreencast(state) {
  try {
    await state.conn.send('Page.stopScreencast', {}, state.sessionId);
  } catch {}
  state.conn.close();
}

/** Writes the manifest to the spool and archives the recording; a failed upload keeps the spool. */
async function saveManifest(manifest, dir) {
  await writeFile(join(dir, 'manifest.json'), JSON.stringify(manifest));
  await archiveRecording(manifest, dir).catch((e) => console.error('[recording] local spool retained:', e.message));
}

/** The finished recording's manifest. */
function manifestOf(sessionId, state) {
  return { ...header(sessionId, state), ...timing(state), ...contents(state) };
}

/** Whose recording it is. A recording is a picture of someone's browser. Ownership travels with it. */
function header(sessionId, state) {
  return { sessionId, owner: state.owner, provider: state.provider, profile: state.profile || null };
}

/** When it started and how long it ran. */
function timing(state) {
  return { startedAt: new Date(state.startedAt).toISOString(), durationMs: Date.now() - state.startedAt };
}

/** Its frames, and whether a cap cut it short. */
function contents(state) {
  return { frameCount: state.frames.length, bytes: state.bytes, truncated: isFull(state), frames: state.frames };
}
