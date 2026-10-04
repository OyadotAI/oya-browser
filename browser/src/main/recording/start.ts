/**
 * Starting (or resuming) a recording: reset the state, note where the person
 * started, arm every tab, and start refreshing the shell.
 */
import { WEB_URL } from '../tabs/constants.ts';
import type { PageView } from '../cdp/cdp.ts';
import { RECORDING_REFRESH_MS } from './constants.ts';
import type { Recorder, RecordingOrigin } from './recorder.ts';
import type { RecordedStep, RecordingResult, RecordingTab } from './types.ts';

/** A workspace draft, as far as a start reads it. */
interface Draft {
  /** The draft's id. */
  id?: string;
  /** Its steps. */
  steps: RecordedStep[];
  /** Its secret field names. */
  secrets: string[];
}

/** The change a fresh recording makes to the workspace. */
interface DraftEdit {
  /** 'new': a fresh draft. */
  type: string;
}

/** The workflow studio, as far as a start uses it. */
interface StartWorkspace {
  /** The draft being edited. */
  draft: Draft;
  /** Whether a validation is running. */
  busy(): boolean;
  /** Applies an edit; a start uses `{ type: 'new' }`. */
  edit(change: DraftEdit): void;
}

/** The stages of a start (reset, mark where the person is, arm every tab, go live), and where the last pause left each tab. */
export class RecordingStart {
  /** The recording being started. */
  private readonly recorder: Recorder;
  /** tab id -> the page recording was paused on */
  private readonly pausedUrls = new Map<number | null, string>();
  /** The draft the pause belongs to; `pausedUrls` mean nothing for another. */
  private pausedDraft: string | undefined = undefined;

  /** `recorder` is the recording this starts. */
  constructor(recorder: Recorder) {
    this.recorder = recorder;
  }

  /** Starts, or resumes, recording on every tab. */
  async start(resume: boolean, origin: RecordingOrigin): Promise<RecordingResult> {
    if (this.recorder.recording) return { recording: true, steps: this.recorder.recordedSteps };
    const workspace: StartWorkspace | null | undefined = this.recorder.deps.workspace;
    if (workspace?.busy()) throw new Error('Stop validation before recording');
    // A fresh recording gets a fresh draft, unless the current one is still empty
    // (just made with "New workflow"): a second one left the empty draft behind in
    // the library as another "Untitled workflow".
    if (!resume && workspace?.draft.steps.length) workspace.edit({ type: 'new' });
    this.reset(resume, origin, workspace);
    this.markStart(resume);
    return this.goLive();
  }

  /** Resets the recording state, keeping the draft's steps on a resume. */
  private reset(resume: boolean, origin: RecordingOrigin, workspace: StartWorkspace | null | undefined): void {
    const recorder = this.recorder;
    recorder.recording = true;
    recorder.recordingOrigin = origin;
    recorder.recordingCutoff = Infinity;
    this.takeDraft(resume && workspace ? workspace.draft : null);
    this.nameTabs(resume);
    recorder.checks.rememberPages();
  }

  /** Takes a resumed draft's steps and secrets, or none for a fresh recording. */
  private takeDraft(draft: Draft | null): void {
    this.recorder.recordedSteps = draft ? structuredClone(draft.steps) : [];
    this.recorder.recordedSecrets = new Set(draft ? draft.secrets : []);
  }

  /** Re-takes step ids and names the active tab. */
  private nameTabs(resume: boolean): void {
    const recorder = this.recorder;
    this.retakeIds();
    if (!resume) {
      recorder.names.clear();
      this.pausedUrls.clear();
    }
    const resumedTab = resume ? recorder.recordedSteps.at(-1)?.tab || 'main' : 'main';
    if (!recorder.names.size) recorder.names.set(recorder.deps.tabs.activeTabId, resumedTab);
  }

  /** Takes the ids of the steps already recorded, so their events are not recorded twice. */
  private retakeIds(): void {
    this.recorder.recordedIds.clear();
    for (const step of this.recorder.recordedSteps) this.recorder.recordedIds.add(step.id);
  }

  /**
   * Replay has to start where the person started, the way an ask() run does, and when
   * they browsed somewhere else while recording was paused, replay has to follow them
   * there, or every step after the resume runs against the page the pause left behind.
   */
  private markStart(resume: boolean): void {
    const recorder = this.recorder;
    const url: string | undefined = recorder.deps.tabs.getActiveView()?.webContents.getURL();
    if (!url || !WEB_URL.test(url)) return;
    if (!resume) recorder.pushRecordedStep({ action: 'navigate', url, start: true });
    else if (this.resumedElsewhere(url)) recorder.pushRecordedStep({ action: 'navigate', url });
  }

  /**
   * Whether a resumed draft picks up on a page other than where it stopped. The pages
   * remembered at the last pause belong to the draft recorded then; for any other
   * draft (opened from the library, or after a restart) where it stopped is unknown,
   * so the page is recorded, unless the draft already ends by going there.
   */
  private resumedElsewhere(url: string): boolean {
    const recorder = this.recorder;
    const tabId = recorder.deps.tabs.activeTabId;
    const known = this.pausedDraft === recorder.deps.workspace?.draft.id && this.pausedUrls.has(tabId);
    if (known) return this.pausedUrls.get(tabId) !== url;
    const last = recorder.recordedSteps.at(-1);
    return !(last?.action === 'navigate' && last.url === url);
  }

  /** Where the pause left each tab, so a resume elsewhere records the move. */
  rememberPausedPages(): void {
    this.pausedDraft = this.recorder.deps.workspace?.draft.id;
    for (const tab of this.recorder.deps.tabs.list as RecordingTab[]) {
      try {
        this.pausedUrls.set(tab.id, tab.view.webContents.getURL());
      } catch {}
    }
  }

  /** Arms every tab, then keeps the shell's step list fresh. */
  private async goLive(): Promise<RecordingResult> {
    await this.armEveryTab();
    this.recorder.drainTimer = setInterval(() => this.safeEmit(), RECORDING_REFRESH_MS);
    this.recorder.emitRecording();
    return { recording: true, steps: this.recorder.recordedSteps };
  }

  /** A refresh that throws is logged: an exception in a timer would bring down the main process. */
  private safeEmit(): void {
    try {
      this.recorder.emitRecording();
    } catch (err) {
      console.error('[recorder] refresh failed:', (err as Error).message);
    }
  }

  /**
   * Arms every tab. The tab the person is looking at has to record, so its failure
   * stops the recording and is thrown. Any other tab that will not arm (hung, or
   * showing an alert) is left out and said so: it joins on its next load, and one
   * stuck background tab no longer made Start look like it stopped at once.
   */
  private async armEveryTab(): Promise<void> {
    const active = this.recorder.deps.tabs.getActiveView();
    for (const tab of this.recorder.deps.tabs.list as RecordingTab[]) {
      const failure = await this.armTab(tab.view);
      if (failure && tab.view === active) return this.abortStart(failure);
      if (failure) console.error(`[recording] tab ${tab.id} is not recorded yet:`, failure.message);
    }
  }

  /** Ends a recording that could not start on the active tab, and throws why. */
  private async abortStart(failure: Error): Promise<never> {
    await this.recorder.stopRecording();
    throw failure;
  }

  /** Arms one tab and answers its failure, if any; one that closed meanwhile is simply left out. */
  private async armTab(view: PageView): Promise<Error | null> {
    try {
      await this.recorder.channels.armRecordingView(view);
      return null;
    } catch (err) {
      return view.webContents.isDestroyed() ? null : (err as Error);
    }
  }
}
