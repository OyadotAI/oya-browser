/** A demonstrated flow, retained until a fresh recording replaces it. */

import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_STEPS, RECORD_MAX_MS, RECORD_POLL_MS } from './constants.ts';

/** Recordings by browser id. */
const active = new Map();

/** Whether the browser is recording right now. */
export const isRecording = (browserId) => !!active.get(browserId)?.recording;
/** The recording as callers see it: every step so far in time order, and the secret names. */
const snapshot = (state) => ({
  recording: state.recording,
  startedAt: state.startedAt,
  steps: [...state.client, ...state.manual].sort((a, b) => (a.t || 0) - (b.t || 0)).slice(0, MAX_STEPS),
  secrets: [...state.secrets],
});

/**
 * Pull new steps from the browser's recorder into the buffer. Serialized with
 * stop/start: a late status response must never replace the final buffer or
 * drain a newer recording into the previous one.
 */
function poll(state, dispatch, mode = 'drain') {
  const work = state.pending.then(() => pull(state, dispatch, mode));
  state.pending = work.catch(() => {});
  return work;
}

/** Asks the browser's recorder for what it has, in `mode`, and buffers it. */
async function pull(state, dispatch, mode) {
  if (!state.recording && mode !== 'start') return snapshot(state);
  const r = await dispatch('record', { mode });
  if (!r?.ok) throw new HttpError(Status.UNPROCESSABLE, r?.error || 'The browser could not record');
  absorb(state, r.data);
  if (mode === 'stop') state.recording = false;
  return snapshot(state);
}

/** Adds steps not seen before, up to MAX_STEPS, and the secret names. */
function absorb(state, data) {
  for (const step of data?.steps || []) {
    const key = step.id || JSON.stringify(step);
    if (state.seen.has(key) || state.client.length >= MAX_STEPS) continue;
    state.seen.add(key);
    state.client.push(step);
  }
  for (const name of data?.secrets || []) state.secrets.add(name);
}

/** A recording with nothing in it yet. */
function freshState() {
  const buffers = { client: [], manual: [], secrets: new Set(), seen: new Set() };
  return { recording: true, ...buffers, startedAt: new Date().toISOString(), timer: null, pending: Promise.resolve() };
}

/** Begin recording on a browser and poll it every 5s, stopping on its own after 30 minutes. Already recording returns the current buffer. */
export async function start(browserId, dispatch) {
  const previous = active.get(browserId);
  if (previous?.recording) return current(previous);
  if (previous) clearTimeout(previous.expiry);
  const state = freshState();
  active.set(browserId, state);
  await begin(browserId, state, dispatch);
  schedule(browserId, state, dispatch);
  return snapshot(state);
}

/** The running recording once its in-flight poll settles. */
async function current(state) {
  await state.pending;
  return snapshot(state);
}

/** The first poll starts the browser's recorder; if it fails, the recording is dropped. */
async function begin(browserId, state, dispatch) {
  try {
    await poll(state, dispatch, 'start');
  } catch (err) {
    active.delete(browserId);
    throw err;
  }
}

/** Polls every RECORD_POLL_MS until the recording has run RECORD_MAX_MS, then stops it. */
function schedule(browserId, state, dispatch) {
  state.timer = setInterval(() => {
    if (Date.now() - Date.parse(state.startedAt) > RECORD_MAX_MS) return void stop(browserId, dispatch).catch(() => {});
    poll(state, dispatch).catch(() => {});
  }, RECORD_POLL_MS);
  state.timer.unref?.();
}

/** Stop recording and return the final steps, kept for 30 minutes afterwards. Null when nothing was recorded. */
export async function stop(browserId, dispatch) {
  const state = active.get(browserId);
  if (!state) return null;
  const final = await poll(state, dispatch, 'stop');
  clearInterval(state.timer);
  expireLater(browserId, state);
  return final;
}

/** Forgets the stopped recording after RECORD_MAX_MS, unless a newer one replaced it. */
function expireLater(browserId, state) {
  clearTimeout(state.expiry);
  state.expiry = setTimeout(() => {
    if (active.get(browserId) === state) active.delete(browserId);
  }, RECORD_MAX_MS);
  state.expiry.unref?.();
}

/** The recording so far; with `dispatch` and still recording, polls the browser first. */
export async function status(browserId, dispatch?) {
  const state = active.get(browserId);
  if (!state) return { recording: false, steps: [], secrets: [] };
  if (dispatch && state.recording) return poll(state, dispatch);
  return snapshot(state);
}

/** Add an http(s) navigate sent through the API to the recording, since the browser's recorder does not see it. */
export function noteCommand(browserId, action, params: any = {}) {
  const state = active.get(browserId);
  if (!state?.recording || action !== 'navigate' || !/^https?:\/\//i.test(params.url || '')) return;
  if (state.manual.length < MAX_STEPS) state.manual.push({ action: 'navigate', url: params.url, t: Date.now() });
}

/** Drop a stopped recording. Refuses while it is still recording. */
export async function discard(browserId) {
  const state = active.get(browserId);
  if (!state) return { ok: true };
  await state.pending;
  if (state.recording) throw new HttpError(Status.CONFLICT, 'Stop the recording before discarding it');
  clearTimeout(state.expiry);
  active.delete(browserId);
  return { ok: true };
}
