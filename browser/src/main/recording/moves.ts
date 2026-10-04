/**
 * Steps for where a person sends a tab themselves: the address bar, a new tab,
 * Back and Forward. The page's typing is collected before each: leaving by the
 * browser's own controls never fires the page's goodbye in time, and the text
 * typed just before was lost.
 */
import { WEB_URL } from '../tabs/constants.ts';
import type { Recorder } from './recorder.ts';
import type { RecordedStep } from './types.ts';

/**
 * The start step exists so a replay begins where the person began. When the
 * first thing they do is go somewhere else, nothing happened on that page, and
 * keeping it sends every replay on a detour through it first.
 */
function dropUnusedStart(steps: RecordedStep[]): void {
  if (steps.length === 1 && steps[0].start) steps.pop();
}

/** The address bar, a new tab, Back and Forward, as steps of the recording. */
export class RecordingMoves {
  /** The recording the moves go into. */
  private readonly recorder: Recorder;

  /** `recorder` is the recording the moves are added to. */
  constructor(recorder: Recorder) {
    this.recorder = recorder;
  }

  /** Collects every page's typing; a page that cannot answer still lets the move be recorded. */
  private collectTyping(): Promise<void> {
    return this.recorder.drainAll(true).catch(() => {});
  }

  /**
   * Only a URL the person asked for, the address bar, a new tab. Where a click or a
   * form submission lands is already the click's step, and a goto over it replays past
   * whatever that click set up (and pins a one-off session URL into the playbook).
   */
  async recordNavigation(url: string): Promise<void> {
    const recorder = this.recorder;
    if (!recorder.recording || !WEB_URL.test(url || '')) return;
    await this.collectTyping();
    const last = recorder.recordedSteps.at(-1);
    const sameTab = () => last?.tab === recorder.names.recordingTab(recorder.deps.tabs.activeTabId);
    if (last && last.action === 'navigate' && last.url === url && sameTab()) return;
    dropUnusedStart(recorder.recordedSteps);
    recorder.pushRecordedStep({ action: 'navigate', url });
  }

  /** Back or forward in the active tab's history. */
  async recordHistory(action: string): Promise<void> {
    if (!this.recorder.recording) return;
    await this.collectTyping();
    this.recorder.pushRecordedStep({ action });
  }
}
