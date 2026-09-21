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
const { drivenElsewhere } = require('../../control-state.cjs');
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

/** Puts the steps from index `from` on in time order, leaving the ones before it where they are. */
function sortFrom(steps, from) {
  const fresh = steps.splice(from).sort((a, b) => (a.t ?? 0) - (b.t ?? 0));
  steps.push(...fresh);
}

/** Where the pause left each tab, so a resume elsewhere records the move. */
function rememberPausedPages(recorder) {
  recorder.pausedDraft = recorder.ctx.workspace?.draft.id;
  for (const tab of recorder.ctx.tabs.list) {
    try {
      recorder.pausedUrls.set(tab.id, tab.view.webContents.getURL());
    } catch {}
  }
}

/**
 * The start step exists so a replay begins where the person began. When the
 * first thing they do is go somewhere else, nothing happened on that page, and
 * keeping it sends every replay on a detour through it first.
 */
function dropUnusedStart(steps) {
  if (steps.length === 1 && steps[0].start) steps.pop();
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
   * Runs `work` after every recording task before it. `work` is handed
   * nothing: a start once took the previous task's result as `resume`, so a
   * Start after any stop or save quietly behaved like a Resume.
   */
  queueRecording(work) {
    const next = this.recordingTask.then(() => work());
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

  /** Hands a changed recording to the workspace, which tells the shell. */
  emitRecording() {
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
   * Only a URL the person asked for, the address bar, a new tab. Where a click or a
   * form submission lands is already the click's step, and a goto over it replays past
   * whatever that click set up (and pins a one-off session URL into the playbook).
   */
  recordNavigation(url) {
    if (!this.recording || !WEB_URL.test(url || '')) return;
    const last = this.recordedSteps[this.recordedSteps.length - 1];
    const sameTab = () => last.tab === this.names.recordingTab(this.ctx.tabs.activeTabId);
    if (last && last.action === 'navigate' && last.url === url && sameTab()) return;
    dropUnusedStart(this.recordedSteps);
    this.pushRecordedStep({ action: 'navigate', url });
  }

  /**
   * Steps a page's channel delivered; a tab's first steps start with where it
   * was. `tabId` is the tab the channel was made for, so a tab that closed with
   * typing still held keeps its name; without it, a view no tab owns is dropped
   * rather than given an invented name.
   */
  receive(view, startingUrl, out, tabId) {
    const id = tabId ?? this.ctx.tabs.list.find((t) => t.view === view)?.id;
    if (!this.recording || id === undefined) return;
    for (const name of out.secrets || []) this.recordedSecrets.add(name);
    const tabName = this.names.recordingTab(id);
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

  /** A new tab joins a recording in progress; queued, so it never slips in while a stop is running. */
  joinIfRecording(view) {
    return this.queueRecording(() => this.recording && this.channels.armRecordingView(view));
  }

  /** Collect what one page buffered. */
  async drainView(view, final = false) {
    if (view) await this.channels.drain(view, final);
  }

  /**
   * Collects every tab and updates the shell. Only the newly drained steps are
   * put in time order: the ones already there are in the order the person left
   * them, and sorting those undid every move they made.
   */
  async drainAll(final = false) {
    const from = this.recordedSteps.length;
    for (const tab of this.ctx.tabs.list) await this.drainView(tab.view, final);
    sortFrom(this.recordedSteps, from);
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
    await this.collectLast();
    return { recording: false, steps: this.recordedSteps };
  }

  /** Stops every page and collects its last steps; the recording ends whatever happens. */
  async collectLast() {
    try {
      await this.channels.stopAll();
      await this.drainAll(true);
    } finally {
      this.settleStopped();
    }
  }

  /** The recording is over, even when a page refused to stop: never left half torn down. */
  settleStopped() {
    this.recording = false;
    rememberPausedPages(this);
    this.emitRecording();
  }

  /** Stops refreshing the shell's step list. */
  stopRefresh() {
    clearInterval(this.drainTimer);
    this.drainTimer = null;
  }

  /** Someone else drives the page now (control-state.cjs): a recording the desktop started ends here. */
  controlLost(state) {
    if (!this.recording || this.recordingOrigin !== 'desktop') return;
    if (!drivenElsewhere(state) || this.recordingCutoff !== Infinity) return;
    this.recordingCutoff = Date.now();
    this.queueRecording(() => this.stopRecording()).catch(() => {});
  }

  /** The server's `record` command: start, stop, or just collect. */
  async remote(mode) {
    if (mode === 'start') await this.startRecording(false, 'remote');
    else if (mode === 'stop') await this.stopRecording();
    else await this.drainAll();
    const { recording, recordingOrigin: origin } = this;
    return { recording, origin, steps: [...this.recordedSteps], secrets: [...this.recordedSecrets] };
  }
}

module.exports = { Recorder };
