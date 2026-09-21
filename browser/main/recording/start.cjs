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
  if (!resume && workspace) workspace.edit({ type: 'new' });
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
  const { pausedUrls } = recorder;
  const activeTabId = recorder.ctx.tabs.activeTabId;
  const url = recorder.ctx.tabs.getActiveView()?.webContents.getURL();
  if (!WEB_URL.test(url || '')) return;
  if (!resume) recorder.pushRecordedStep({ action: 'navigate', url, start: true });
  else if (pausedUrls.has(activeTabId) && pausedUrls.get(activeTabId) !== url)
    recorder.pushRecordedStep({ action: 'navigate', url });
}

/** Arms every tab, then keeps the shell's step list fresh. */
async function goLiveRecording(recorder) {
  await armEveryTab(recorder);
  recorder.drainTimer = setInterval(() => recorder.emitRecording(), RECORDING_REFRESH_MS);
  recorder.emitRecording();
  return { recording: true, steps: recorder.recordedSteps };
}

/** Arms every tab; a failure stops the recording and is thrown. */
async function armEveryTab(recorder) {
  try {
    for (const tab of recorder.ctx.tabs.list) await recorder.channels.armRecordingView(tab.view);
  } catch (err) {
    await recorder.stopRecording();
    throw err;
  }
}

module.exports = { startRecording };
