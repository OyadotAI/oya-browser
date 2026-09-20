/**
 * Recording: a person demonstrates the task, the server keeps it as a playbook.
 *
 * The page buffers what the user does (scripts/analyzer.js) in the same step shape the
 * agent produces, so a recording gets replay, healing and the Playwright export for
 * free from server/src/modules/playbooks/service.ts. Nothing here interprets the steps.
 */
const { normalizeStep, TARGETED } = require('../../scripts/workflow.cjs');
const { RecordingChannels } = require('./channels.cjs');
const { RecordingTabNames } = require('./tab-names.cjs');
const { WEB_URL } = require('../tabs/constants.cjs');
const { startRecording } = require('./start.cjs');
const { MAX_RECORDED_STEPS, FILE_PICKER_CLICK_MS } = require('./constants.cjs');

/** A recorded step normalized, or undefined when it is malformed: logged, never thrown, since a throw would stop the tab's later steps. */
function safeStep(raw) {
  try {
    return parkUntargeted(normalizeStep(raw));
  } catch (err) {
    console.error('[recording] dropped a step:', err.message);
    return undefined;
  }
}

/**
 * A step with nothing to find its element by would stop the whole workflow from
 * saving or validating. It is kept, turned off, with the reason, for the person
 * to pick a target or delete.
 */
function parkUntargeted(step) {
  if (!TARGETED.includes(step.action) || step.candidates.length) return step;
  return {
    ...step,
    enabled: false,
    captureIssue: 'Nothing identifies this element. Pick a target, or delete the step.',
  };
}

/**
 * The click that opened a file picker, once its upload is recorded, is turned
 * off: the upload sets the file itself, and replaying the click would open the
 * operating system's dialog. It stays in the list for the person to turn back on.
 */
function disablePickerClick(steps) {
  const i = steps.length - 1;
  const last = steps[i];
  const prev = steps[i - 1];
  if (last?.action !== 'upload_file' || prev?.action !== 'click' || prev.tab !== last.tab) return;
  if (last.t - prev.t < FILE_PICKER_CLICK_MS) prev.enabled = false;
}

/** The recording in progress (or paused) and its steps. */
class Recorder {
  /** Whether a recording is running. */
  recording = false;
  /** Who started it: 'desktop' (the panel) or 'remote' (the server). */
  recordingOrigin = 'desktop';
  /** Steps after this time are dropped: control passed to an agent. */
  recordingCutoff = Infinity;
  /** The steps so far. */
  recordedSteps = [];
  /** Names of secret fields typed into. */
  recordedSecrets = new Set();
  /** Step ids already taken, so a replayed event is not recorded twice. */
  recordedIds = new Set();
  /** tab id -> the page recording was paused on */
  pausedUrls = new Map();
  /** Refreshes the shell's step list while recording. */
  drainTimer = null;
  /** Serializes start, stop, clear and save. */
  recordingTask = Promise.resolve();
  /** The last state handed to the workspace, to skip identical captures. */
  captureSignature = '';

  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Per-tab recording channels. */
    this.channels = new RecordingChannels(ctx);
    /** Tab id → recorded tab name. */
    this.names = new RecordingTabNames(() => this.recordedSteps);
  }

  /**
   * Runs `work` after every recording task before it. `work` receives the
   * previous task's result, as it always has.
   */
  queueRecording(work) {
    const next = this.recordingTask.then(work);
    this.recordingTask = next.catch(() => {});
    return next;
  }

  /** Identifies what the workspace last saw. */
  signature() {
    return JSON.stringify([this.recording, this.recordedSteps, [...this.recordedSecrets]]);
  }

  /** Takes a draft's steps and secrets as the recording's own. */
  adopt(steps, secrets) {
    this.recordedSteps = steps;
    this.recordedSecrets = new Set(secrets);
    this.captureSignature = this.signature();
  }

  /** Tells the shell the steps, and hands a changed recording to the workspace. */
  emitRecording() {
    this.ctx.shell.send('recorded-steps', { recording: this.recording, steps: this.recordedSteps });
    const signature = this.signature();
    const workspace = this.ctx.workspace;
    if (!workspace || signature === this.captureSignature) return;
    this.captureSignature = signature;
    workspace.capture(this.recordedSteps, this.recordedSecrets, this.recording);
  }

  /** Adds one step, unless it is late, a duplicate, or over the limit. */
  pushRecordedStep(step) {
    if (!this.recording || (step.t || Date.now()) > this.recordingCutoff) return;
    if (this.recordedSteps.length >= MAX_RECORDED_STEPS) return this.stepLimitReached();
    if (step.id && this.recordedIds.has(step.id)) return;
    if (step.id) this.recordedIds.add(step.id);
    const normalized = safeStep({ t: Date.now(), tab: this.names.recordingTab(this.ctx.tabs.activeTabId), ...step });
    if (!normalized) return;
    this.recordedSteps.push(normalized);
    disablePickerClick(this.recordedSteps);
  }

  /** Marks the last step and stops. */
  stepLimitReached() {
    this.recordedSteps[MAX_RECORDED_STEPS - 1].captureIssue =
      'The 500-step capture limit was reached. Later actions were not recorded. Split this workflow and review its ending.';
    this.queueRecording(() => this.stopRecording());
  }

  /**
   * Only a URL the person asked for — the address bar, a new tab. Where a click or a
   * form submission lands is already the click's step, and a goto over it replays past
   * whatever that click set up (and pins a one-off session URL into the playbook).
   */
  recordNavigation(url) {
    if (!this.recording || !WEB_URL.test(url || '')) return;
    const last = this.recordedSteps[this.recordedSteps.length - 1];
    const sameTab = () => last.tab === this.names.recordingTab(this.ctx.tabs.activeTabId);
    if (last && last.action === 'navigate' && last.url === url && sameTab()) return;
    this.pushRecordedStep({ action: 'navigate', url });
  }

  /** Steps a page's channel delivered; a tab's first steps start with where it was. */
  receive(view, startingUrl, out) {
    if (!this.recording) return;
    for (const name of out.secrets || []) this.recordedSecrets.add(name);
    const owner = this.ctx.tabs.list.find((t) => t.view === view);
    const tabName = this.names.recordingTab(owner?.id);
    // A new tab's channel is made while it is still about:blank; its first steps
    // happened on the page it is showing now.
    const start = WEB_URL.test(startingUrl || '') ? startingUrl : view.webContents?.getURL?.();
    this.markTabStart(tabName, start || '', out.steps);
    for (const step of out.steps || []) this.pushRecordedStep({ ...step, tab: tabName });
  }

  /** A tab's first recorded steps are preceded by the page it started on. */
  markTabStart(tabName, startingUrl, steps) {
    const fresh = !this.recordedSteps.some((step) => step.tab === tabName);
    if (!steps?.length || !fresh || !WEB_URL.test(startingUrl)) return;
    this.pushRecordedStep({ action: 'navigate', url: startingUrl, tab: tabName, t: (steps[0].t || Date.now()) - 1 });
  }

  /** A new tab joins a recording in progress. */
  joinIfRecording(view) {
    if (this.recording) return this.channels.armRecordingView(view);
  }

  /** Collect what one page buffered. Steps carry their own timestamps; the merge sorts by them. */
  async drainView(view, final = false) {
    if (!view) return;
    await this.channels.drain(view, final);
    this.recordedSteps.sort((a, b) => a.t - b.t);
  }

  /** Collects every tab and updates the shell. */
  async drainAll(final = false) {
    for (const tab of this.ctx.tabs.list) await this.drainView(tab.view, final);
    this.emitRecording();
  }

  /** Starts, or resumes, recording on every tab (see start.cjs). */
  startRecording(resume = false, origin = 'desktop') {
    return startRecording(this, resume, origin);
  }

  /** Stops recording, collects the last steps, and remembers where each tab was. */
  async stopRecording() {
    if (!this.recording) return { recording: false, steps: this.recordedSteps };
    this.stopRefresh();
    await this.channels.stopAll();
    await this.drainAll(true);
    this.recording = false;
    this.rememberPausedPages();
    this.emitRecording();
    return { recording: false, steps: this.recordedSteps };
  }

  /** Stops refreshing the shell's step list. */
  stopRefresh() {
    clearInterval(this.drainTimer);
    this.drainTimer = null;
  }

  /** Where the pause left each tab, so a resume elsewhere records the move. */
  rememberPausedPages() {
    for (const tab of this.ctx.tabs.list) {
      try {
        this.pausedUrls.set(tab.id, tab.view.webContents.getURL());
      } catch {}
    }
  }

  /** Control passed to an agent: a recording the desktop started ends here. */
  controlLost(state) {
    if (!this.recording || this.recordingOrigin !== 'desktop') return;
    if (state.interactive || this.recordingCutoff !== Infinity) return;
    this.recordingCutoff = Date.now();
    this.queueRecording(() => this.stopRecording()).catch(() => {});
  }

  /** Throws the steps away (a recording keeps running, from the current page). */
  async clear() {
    await this.channels.clearAll();
    this.recordedSteps = [];
    this.recordedSecrets = new Set();
    this.recordedIds.clear();
    const url = this.ctx.tabs.getActiveView()?.webContents.getURL();
    if (this.recording && WEB_URL.test(url || '')) this.pushRecordedStep({ action: 'navigate', url, start: true });
    this.emitRecording();
    return { recording: this.recording, steps: this.recordedSteps };
  }

  /** The server's `record` command: start, stop, or just collect. */
  async remote(mode) {
    if (mode === 'start') await this.startRecording(false, 'remote');
    else if (mode === 'stop') await this.stopRecording();
    else await this.drainAll();
    return { recording: this.recording, steps: [...this.recordedSteps], secrets: [...this.recordedSecrets] };
  }
}

module.exports = { Recorder };
