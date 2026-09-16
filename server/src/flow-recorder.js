/**
 * A flow demonstrated by a person, as playbook steps.
 *
 * The page buffers what the user does (browser/scripts/analyzer.js) and this polls it
 * out over the same command channel everything else uses, so a live-view session —
 * where every click is a pair of coordinates — comes back as element steps that
 * replay. The shape is an ask() run's, so playbook.js stores, replays and exports it
 * with no idea a person produced it.
 *
 * ponytail: in memory per browser and dropped on restart. A recording lasts minutes
 * and ends in a save; persistence would only matter across a deploy.
 */

const POLL_MS = 5000;   // a safety net; the dashboard's own status call is what keeps the list fresh
const MAX_MS = 30 * 60 * 1000;   // a recording nobody stopped is not a recording
const MAX_STEPS = 500;

const active = new Map(); // browserId -> { client, manual, secrets, timer, startedAt }

export const isRecording = (browserId) => active.has(browserId);

const snapshot = (state) => ({
  recording: true,
  startedAt: state.startedAt,
  steps: [...state.client, ...state.manual].sort((a, b) => (a.t || 0) - (b.t || 0)).slice(0, MAX_STEPS),
  secrets: [...state.secrets],
});

/**
 * The browser answers with everything it has buffered, not a delta, so this merges
 * rather than replaces: a client that reattached to a new target and started its
 * buffer again must not take the first half of the recording with it.
 */
async function poll(browserId, dispatch, mode = 'drain') {
  const r = await dispatch('record', { mode });
  if (!r?.ok) throw Object.assign(new Error(r?.error || 'The browser could not record'), { status: 422 });
  const state = active.get(browserId);
  if (!state) return null;
  for (const step of r.data?.steps || []) {
    const key = `${step.t}|${step.action}|${step.url || step.text || step.option || step.key || ''}`;
    if (state.seen.has(key)) continue;
    state.seen.add(key);
    state.client.push(step);
  }
  for (const name of r.data?.secrets || []) state.secrets.add(name);
  return snapshot(state);
}

export async function start(browserId, dispatch) {
  if (active.has(browserId)) return snapshot(active.get(browserId));
  const state = { client: [], manual: [], secrets: new Set(), seen: new Set(), startedAt: new Date().toISOString(), timer: null };
  active.set(browserId, state);
  try {
    await poll(browserId, dispatch, 'start');
  } catch (err) {
    active.delete(browserId);
    throw err;
  }
  // Polled rather than pushed: the page has no way back to the server, and the
  // buffer dies with the document it lives in.
  state.timer = setInterval(() => {
    if (Date.now() - Date.parse(state.startedAt) > MAX_MS) return void stop(browserId, dispatch).catch(() => {});
    poll(browserId, dispatch).catch(() => {});
  }, POLL_MS);
  state.timer.unref?.();
  return snapshot(state);
}

/** The steps stay until the caller saves or discards them — stopping is not throwing away. */
export async function stop(browserId, dispatch) {
  const state = active.get(browserId);
  if (!state) return null;
  clearInterval(state.timer);
  const final = await poll(browserId, dispatch, 'stop').catch(() => snapshot(state));
  active.delete(browserId);
  return { ...final, recording: false };
}

/**
 * Asking is collecting: whoever is watching drives the freshness, so a step shows up
 * one round trip after it happens rather than after a timer nobody is waiting on.
 */
export async function status(browserId, dispatch) {
  const state = active.get(browserId);
  if (!state) return { recording: false, steps: [], secrets: [] };
  if (dispatch) return (await poll(browserId, dispatch).catch(() => null)) || snapshot(state);
  return snapshot(state);
}

/**
 * A command the person asked for through the live view. Only navigation: a click or a
 * key already reaches the page, where the recorder sees it as the event it really was.
 */
export function noteCommand(browserId, action, params = {}) {
  const state = active.get(browserId);
  if (!state || action !== 'navigate' || !/^https?:\/\//i.test(params.url || '')) return;
  state.manual.push({ action: 'navigate', url: params.url, t: Date.now() });
}
