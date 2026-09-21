/**
 * Starting (or resuming) a recording: reset the state, note where the person
 * started, arm every tab, and start refreshing the shell.
 */
const { WEB_URL } = require('../tabs/constants.cjs');
const { RECORDING_REFRESH_MS } = require('./constants.cjs');

/** Starts, or resumes, recording on every tab. */
async function startRecording(recorder, resume, origin) {
  if (recorder.recording) return { recording: true, steps: recorder.recordedSteps };
  const workspace = recorder.ctx.workspace;
  if (workspace?.busy()) throw new Error('Stop validation before recording');
  // A fresh recording gets a fresh draft, unless the current one is still empty
  // (just made with "New workflow"): a second one left the empty draft behind in
  // the library as another "Untitled workflow".
  if (!resume && workspace?.draft.steps.length) workspace.edit({ type: 'new' });
  resetRecording(recorder, resume, origin, workspace);
  markRecordingStart(recorder, resume);
  return goLiveRecording(recorder);
}

/** Resets the recording state, keeping the draft's steps on a resume. */
function resetRecording(recorder, resume, origin, workspace) {
  recorder.recording = true;
  recorder.recordingOrigin = origin;
  recorder.recordingCutoff = Infinity;
  const draft = resume && workspace ? workspace.draft : null;
  recorder.recordedSteps = draft ? structuredClone(draft.steps) : [];
  recorder.recordedSecrets = new Set(draft ? draft.secrets : []);
  nameRecordingTabs(recorder, resume);
}

/** Re-takes step ids and names the active tab. */
function nameRecordingTabs(recorder, resume) {
  recorder.recordedIds.clear();
  for (const step of recorder.recordedSteps) recorder.recordedIds.add(step.id);
  if (!resume) {
    recorder.names.clear();
    recorder.pausedUrls.clear();
  }
  const resumedTab = resume ? recorder.recordedSteps.at(-1)?.tab || 'main' : 'main';
  if (!recorder.names.size) recorder.names.set(recorder.ctx.tabs.activeTabId, resumedTab);
}

/**
 * Replay has to start where the person started, the way an ask() run does, and when
 * they browsed somewhere else while recording was paused, replay has to follow them
 * there, or every step after the resume runs against the page the pause left behind.
 */
function markRecordingStart(recorder, resume) {
  const url = recorder.ctx.tabs.getActiveView()?.webContents.getURL();
  if (!WEB_URL.test(url || '')) return;
  if (!resume) recorder.pushRecordedStep({ action: 'navigate', url, start: true });
  else if (resumedElsewhere(recorder, url)) recorder.pushRecordedStep({ action: 'navigate', url });
}

/**
 * Whether a resumed draft picks up on a page other than where it stopped. The pages
 * remembered at the last pause belong to the draft recorded then; for any other
 * draft (opened from the library, or after a restart) where it stopped is unknown,
 * so the page is recorded, unless the draft already ends by going there.
 */
function resumedElsewhere(recorder, url) {
  const tabId = recorder.ctx.tabs.activeTabId;
  const known = recorder.pausedDraft === recorder.ctx.workspace?.draft.id && recorder.pausedUrls.has(tabId);
  if (known) return recorder.pausedUrls.get(tabId) !== url;
  const last = recorder.recordedSteps.at(-1);
  return !(last?.action === 'navigate' && last.url === url);
}

/** Arms every tab, then keeps the shell's step list fresh. */
async function goLiveRecording(recorder) {
  await armEveryTab(recorder);
  recorder.drainTimer = setInterval(() => safeEmit(recorder), RECORDING_REFRESH_MS);
  recorder.emitRecording();
  return { recording: true, steps: recorder.recordedSteps };
}

/** A refresh that throws is logged: an exception in a timer would bring down the main process. */
function safeEmit(recorder) {
  try {
    recorder.emitRecording();
  } catch (err) {
    console.error('[recorder] refresh failed:', err.message);
  }
}

/** Arms every tab; a live tab's failure stops the recording and is thrown. */
async function armEveryTab(recorder) {
  try {
    for (const tab of recorder.ctx.tabs.list) await armTab(recorder, tab.view);
  } catch (err) {
    await recorder.stopRecording();
    throw err;
  }
}

/** Arms one tab; one that closed meanwhile is simply left out. */
async function armTab(recorder, view) {
  try {
    await recorder.channels.armRecordingView(view);
  } catch (err) {
    if (!view.webContents.isDestroyed()) throw err;
  }
}

module.exports = { startRecording };
