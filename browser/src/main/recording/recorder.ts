/**
 * Recording: a person demonstrates the task, the server keeps it as a playbook.
 *
 * The page buffers what the user does (scripts/analyzer.js) in the same step shape the
 * agent produces, so a recording gets replay, healing and the Playwright export for
 * free from server/src/modules/playbooks/service.ts. Nothing here interprets the steps.
 */
import type { AppServices } from '../app/services.ts';
import type { PageView } from '../cdp/cdp.ts';
import { drivenElsewhere, type ControlValue } from '../control/control-state.ts';
import { WEB_URL } from '../tabs/constants.ts';
import { RecordingChannels } from './channels.ts';
import { RecordingTabNames } from './tab-names.ts';
import { RecordingStart } from './start.ts';
import { PageChecks } from './outcomes.ts';
import { RecordingMoves } from './moves.ts';
import { appendStep, safeStep, sortFrom } from './steps.ts';
import { MAX_RECORDED_STEPS } from './constants.ts';
import type { PageOutput, RecordedStep, RecordingResult, RecordingTab } from './types.ts';

/** The services the recorder uses; its channels reach it back through `recorder`. */
type Deps = Pick<AppServices, 'tabs' | 'workspace' | 'isolatedWorld' | 'analyzerScript' | 'recorder'>;

/** Who started a recording: 'desktop' (the panel) or 'remote' (the server). */
export type RecordingOrigin = 'desktop' | 'remote';

/** What the server's `record` command answers. */
export interface RemoteRecording extends RecordingResult {
  /** Who started the recording. */
  origin: RecordingOrigin;
  /** Names of secret fields typed into. */
  secrets: string[];
}

/** The recording in progress (or paused) and its steps. */
export class Recorder {
  /** Whether a recording is running. */
  recording = false;
  /** Who started it. */
  recordingOrigin: RecordingOrigin = 'desktop';
  /** Steps after this time are dropped: control passed to an agent. */
  recordingCutoff = Infinity;
  /** The steps so far. */
  recordedSteps: RecordedStep[] = [];
  /** Names of secret fields typed into. */
  recordedSecrets = new Set<string>();
  /** Step ids already taken, so a replayed event is not recorded twice. */
  readonly recordedIds = new Set<string | undefined>();
  /** Refreshes the shell's step list while recording. */
  drainTimer: ReturnType<typeof setInterval> | null = null;
  /** Serializes start, stop, clear and save. */
  recordingTask: Promise<unknown> = Promise.resolve();
  /** The last state handed to the workspace, to skip identical captures. */
  private captureSignature = '';
  /** The main-process services; the recorder's collaborators read them too. */
  readonly deps: Deps;
  /** Per-tab recording channels. */
  readonly channels: RecordingChannels;
  /** Tab id → recorded tab name. */
  readonly names = new RecordingTabNames(() => this.recordedSteps);
  /** The page checks a person's moves add (outcomes.ts). */
  readonly checks = new PageChecks(this);
  /** The address bar, new tabs, Back and Forward (moves.ts). */
  private readonly moves = new RecordingMoves(this);
  /** The stages of a start, and where the last pause left each tab (start.ts). */
  private readonly starter = new RecordingStart(this);

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.channels = new RecordingChannels(deps);
  }

  /**
   * Runs `work` after every recording task before it. `work` is handed
   * nothing: a start once took the previous task's result as `resume`, so a
   * Start after any stop or save quietly behaved like a Resume.
   */
  queueRecording<T>(work: () => T | Promise<T>): Promise<T> {
    const next = this.recordingTask.then(() => work());
    this.recordingTask = next.catch(() => {});
    return next;
  }

  /** Identifies what the workspace last saw. */
  private signature(): string {
    return JSON.stringify([this.recording, this.recordedSteps, [...this.recordedSecrets]]);
  }

  /** Takes a draft's steps and secrets as the recording's own. */
  adopt(steps: RecordedStep[], secrets: Iterable<string>): void {
    this.recordedSteps = steps;
    this.recordedSecrets = new Set(secrets);
    this.captureSignature = this.signature();
  }

  /** Hands a changed recording to the workspace, which tells the shell. */
  emitRecording(): void {
    const signature = this.signature();
    const workspace = this.deps.workspace;
    if (!workspace || signature === this.captureSignature) return;
    this.captureSignature = signature;
    workspace.capture(this.recordedSteps, this.recordedSecrets, this.recording);
  }

  /** Adds one step, unless it is late, a duplicate, or over the limit. */
  pushRecordedStep(step: RecordedStep): void {
    if (!this.recording || (step.t || Date.now()) > this.recordingCutoff) return;
    if (this.recordedSteps.length >= MAX_RECORDED_STEPS) return this.stepLimitReached();
    if (step.id && this.recordedIds.has(step.id)) return;
    if (step.id) this.recordedIds.add(step.id);
    const normalized = safeStep({ t: Date.now(), tab: this.names.recordingTab(this.deps.tabs.activeTabId), ...step });
    if (normalized) appendStep(this.recordedSteps, normalized);
  }

  /** Marks the last step and stops. */
  private stepLimitReached(): void {
    this.recordedSteps[MAX_RECORDED_STEPS - 1].captureIssue =
      'The 500-step capture limit was reached. Later actions were not recorded. Split this workflow and review its ending.';
    this.queueRecording(() => this.stopRecording());
  }

  /** The address bar or a new tab (moves.ts). */
  recordNavigation(url: string): Promise<void> {
    return this.moves.recordNavigation(url);
  }

  /** Back or forward in the tab's history (moves.ts). */
  recordHistory(action: string): Promise<void> {
    return this.moves.recordHistory(action);
  }

  /** A tab's main frame moved to `url`: a page check, if a person's action moved it (outcomes.ts). */
  pageReached(tabId: number, url: string): void {
    this.checks.pageReached(tabId, url);
  }

  /**
   * Steps a page's channel delivered; a tab's first steps start with where it
   * was. `tabId` is the tab the channel was made for, so a tab that closed with
   * typing still held keeps its name; without it, a view no tab owns is dropped
   * rather than given an invented name.
   */
  receive(view: PageView, startingUrl: string, out: PageOutput, tabId?: number): void {
    const id = tabId ?? this.deps.tabs.list.find((t: RecordingTab) => t.view === view)?.id;
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
  private markTabStart(tabName: string, startingUrl: string, steps: RecordedStep[] | undefined): void {
    const fresh = !this.recordedSteps.some((step) => step.tab === tabName);
    if (!steps?.length || !fresh || !WEB_URL.test(startingUrl)) return;
    this.pushRecordedStep({ action: 'navigate', url: startingUrl, tab: tabName, t: (steps[0].t || Date.now()) - 1 });
  }

  /** A new tab joins a recording in progress; queued, so it never slips in while a stop is running. */
  joinIfRecording(view: PageView): Promise<unknown> {
    return this.queueRecording(() => this.recording && this.channels.armRecordingView(view));
  }

  /**
   * Collects every tab and updates the shell. Only the newly drained steps are
   * put in time order: the ones already there are in the order the person left
   * them, and sorting those undid every move they made.
   */
  async drainAll(final = false): Promise<void> {
    const from = this.recordedSteps.length;
    for (const tab of this.deps.tabs.list as RecordingTab[]) if (tab.view) await this.channels.drain(tab.view, final);
    sortFrom(this.recordedSteps, from);
    this.emitRecording();
  }

  /** Starts, or resumes, recording on every tab (see start.ts). */
  startRecording(resume = false, origin: RecordingOrigin = 'desktop'): Promise<RecordingResult> {
    return this.starter.start(resume, origin);
  }

  /** Stops recording, collects the last steps, and remembers where each tab was. */
  async stopRecording(): Promise<RecordingResult> {
    if (!this.recording) return { recording: false, steps: this.recordedSteps };
    this.stopRefresh();
    await this.collectLast();
    return { recording: false, steps: this.recordedSteps };
  }

  /** Stops every page and collects its last steps; the recording ends whatever happens. */
  private async collectLast(): Promise<void> {
    try {
      await this.channels.stopAll();
      await this.drainAll(true);
      this.checks.finalPageCheck();
    } finally {
      this.settleStopped();
    }
  }

  /** The recording is over, even when a page refused to stop: never left half torn down. */
  private settleStopped(): void {
    this.recording = false;
    this.starter.rememberPausedPages();
    this.emitRecording();
  }

  /** Stops refreshing the shell's step list. */
  private stopRefresh(): void {
    clearInterval(this.drainTimer ?? undefined);
    this.drainTimer = null;
  }

  /** Someone else drives the page now (control-state.ts): a recording the desktop started ends here. */
  controlLost(state: ControlValue): void {
    if (!this.recording || this.recordingOrigin !== 'desktop') return;
    if (!drivenElsewhere(state) || this.recordingCutoff !== Infinity) return;
    this.recordingCutoff = Date.now();
    this.queueRecording(() => this.stopRecording()).catch(() => {});
  }

  /** The server's `record` command: start, stop, or just collect. */
  async remote(mode: string | undefined): Promise<RemoteRecording> {
    if (mode === 'start') await this.startRecording(false, 'remote');
    else if (mode === 'stop') await this.stopRecording();
    else await this.drainAll();
    const { recording, recordingOrigin: origin } = this;
    return { recording, origin, steps: [...this.recordedSteps], secrets: [...this.recordedSecrets] };
  }
}
