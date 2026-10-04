/** The shapes the recorder's files share: a recorded step, a tab, what a page's channel delivers. */
import type { PageView } from '../cdp/cdp.ts';

/** One recorded step, as the page and normalizeStep shape it; fields the recorder never reads pass through. */
export interface RecordedStep {
  /** The page's id for the step, so a replayed event is not recorded twice. */
  id?: string;
  /** What the step does: 'click', 'navigate', 'assert_page', ... */
  action: string;
  /** The recorded tab name ('main', 'tab-1', ...). */
  tab?: string;
  /** When it was captured, in epoch milliseconds; never sent to the server. */
  t?: number;
  /** Where a navigate step goes. */
  url?: string;
  /** Set on the navigate step for where the person began. */
  start?: boolean;
  /** False for a step kept in the list but turned off. */
  enabled?: boolean;
  /** The ways a replay can find the step's element. */
  candidates?: unknown[];
  /** Why the step may not replay as recorded, for the person. */
  captureIssue?: string;
  /** The page an assert_page step expects. */
  expected?: string;
  /** The query parameters an assert_page step holds to, comma separated. */
  params?: string;
  /** Anything else the step carries. */
  [field: string]: unknown;
}

/** A tab as the recorder reads it from the tab manager. */
export interface RecordingTab {
  /** The desktop's tab id. */
  id: number;
  /** The view showing the tab's page. */
  view: PageView;
}

/** What a page's channel delivers: the steps it buffered and the secret fields typed into. */
export interface PageOutput {
  /** The steps, in the page's own shape. */
  steps?: RecordedStep[];
  /** Names of secret fields typed into. */
  secrets?: string[];
}

/** What starting, stopping and the server's `record` command answer. */
export interface RecordingResult {
  /** Whether a recording is running now. */
  recording: boolean;
  /** The recorded steps. */
  steps: RecordedStep[];
}
