/** A demonstrated flow, retained until a fresh recording replaces it. */
const POLL_MS = 5000;
const MAX_MS = 30 * 60 * 1000;
const MAX_STEPS = 500;
const active = new Map();

export const isRecording = (browserId) => !!active.get(browserId)?.recording;
const snapshot = (state) => ({
  recording: state.recording,
  startedAt: state.startedAt,
  steps: [...state.client, ...state.manual].sort((a, b) => (a.t || 0) - (b.t || 0)).slice(0, MAX_STEPS),
  secrets: [...state.secrets],
});

// Serialize polls with stop/start. A late status response must never replace
// the final buffer or drain a newer recording into the previous one.
function poll(state, dispatch, mode = 'drain') {
  const work = state.pending.then(async () => {
    if (!state.recording && mode !== 'start') return snapshot(state);
    const r = await dispatch('record', { mode });
    if (!r?.ok) throw Object.assign(new Error(r?.error || 'The browser could not record'), { status: 422 });
    for (const step of r.data?.steps || []) {
      const key = step.id || JSON.stringify(step);
      if (state.seen.has(key) || state.client.length >= MAX_STEPS) continue;
      state.seen.add(key);
      state.client.push(step);
    }
    for (const name of r.data?.secrets || []) state.secrets.add(name);
    if (mode === 'stop') state.recording = false;
    return snapshot(state);
  });
  state.pending = work.catch(() => {});
  return work;
}

export async function start(browserId, dispatch) {
  const previous = active.get(browserId);
  if (previous?.recording) { await previous.pending; return snapshot(previous); }
  if (previous) clearTimeout(previous.expiry);
  const state = { recording: true, client: [], manual: [], secrets: new Set(), seen: new Set(),
    startedAt: new Date().toISOString(), timer: null, pending: Promise.resolve() };
  active.set(browserId, state);
  try { await poll(state, dispatch, 'start'); }
  catch (err) { active.delete(browserId); throw err; }
  state.timer = setInterval(() => {
    if (Date.now() - Date.parse(state.startedAt) > MAX_MS) return void stop(browserId, dispatch).catch(() => {});
    poll(state, dispatch).catch(() => {});
  }, POLL_MS);
  state.timer.unref?.();
  return snapshot(state);
}

export async function stop(browserId, dispatch) {
  const state = active.get(browserId);
  if (!state) return null;
  const final = await poll(state, dispatch, 'stop');
  clearInterval(state.timer);
  clearTimeout(state.expiry);
  state.expiry = setTimeout(() => { if (active.get(browserId) === state) active.delete(browserId); }, MAX_MS);
  state.expiry.unref?.();
  return final;
}

export async function status(browserId, dispatch) {
  const state = active.get(browserId);
  if (!state) return { recording: false, steps: [], secrets: [] };
  if (dispatch && state.recording) return poll(state, dispatch);
  return snapshot(state);
}

export function noteCommand(browserId, action, params = {}) {
  const state = active.get(browserId);
  if (!state?.recording || action !== 'navigate' || !/^https?:\/\//i.test(params.url || '')) return;
  if (state.manual.length < MAX_STEPS) state.manual.push({ action: 'navigate', url: params.url, t: Date.now() });
}

export async function discard(browserId) {
  const state = active.get(browserId);
  if (!state) return { ok: true };
  await state.pending;
  if (state.recording) throw Object.assign(new Error('Stop the recording before discarding it'), { status: 409 });
  clearTimeout(state.expiry);
  active.delete(browserId);
  return { ok: true };
}
