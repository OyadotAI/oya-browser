/**
 * The shapes the Playbooks tab reads from the server: saved playbooks, replay
 * runs and in-progress recordings.
 */

/** A playbook's replayable body, as saved or as a healed draft. */
export interface PlaybookBody {
  /** The name code passes to play(). */
  name: string;
  /** Fields a caller can override on replay. */
  variables: string[];
  /** The values the recording used, which an untouched field replays with. */
  defaults: Record<string, string>;
  /** How many steps the replay makes. */
  steps: number;
  /** The same flow as a Playwright module. */
  code: string;
}

/** A healed draft: the steps an agent used to finish a broken replay. */
export type PlaybookDraft = PlaybookBody & {
  /** When the agent finished the run. */
  healedAt: string;
  /** The zero-based step where the replay broke. */
  healedFrom: number;
};

/** One row of GET /playbooks. */
export interface PlaybookInfo extends PlaybookBody {
  /** When it was saved. */
  createdAt: string | null;
  /** When a healed draft last replaced its steps. */
  promotedAt: string | null;
  /** A healed draft waiting for review, if any. */
  draft: PlaybookDraft | null;
}

/** Why a run stopped for a person. */
export type AttentionReason = 'captcha' | 'login' | 'mfa' | 'agent' | 'heal_failed';

/** What a paused run needs from a person. */
export interface RunAttention {
  /** The request's id on the server. */
  id: string;
  /** What kind of help it needs. */
  reason: AttentionReason;
  /** What it says to the person. */
  message: string;
  /** Where to watch and act on the browser, when there is one. */
  liveViewUrl?: string;
}

/** How a finished run went. */
export interface RunResult {
  /** Steps replayed without the LLM. */
  steps?: number;
  /** Steps in the playbook. */
  total?: number;
  /** A person finished the run. */
  fellBack?: boolean;
  /** The agent finished the run and saved a draft. */
  healed?: boolean;
  /** The draft's name, when one was saved. */
  draft?: string;
  /** What the run reported back. */
  text?: string;
}

/** Where a run is in its life. */
export type RunStatus = 'running' | 'needs_attention' | 'succeeded' | 'failed';

/** A replay run, as the server reports it. */
export interface RunInfo {
  /** The run's id, for polling and replies. */
  id: string;
  /** Where it is. */
  status: RunStatus;
  /** What it needs from a person, while it is paused. */
  attention: RunAttention | null;
  /** How it went, once it succeeded. */
  result?: RunResult;
  /** Why it failed. */
  error?: string;
}

/** The element a recorded step acted on, by whatever names it had. */
export interface StepElement {
  /** Visible text. */
  text?: string;
  /** Accessible or form name. */
  name?: string;
  /** DOM id. */
  domId?: string;
  /** Test id attribute. */
  testId?: string;
  /** Tag name, the last resort. */
  tag?: string;
}

/** One step the page saw a person make. */
export interface RecordedStep {
  /** navigate, click, type, select_option, press_key… */
  action: string;
  /** Where a navigate went. */
  url?: string;
  /** What was typed. */
  text?: string;
  /** What was picked. */
  option?: string;
  /** Which key was pressed. */
  key?: string;
  /** The element acted on. */
  el?: StepElement;
}

/** A recording's state on the server. */
export interface RecordState {
  /** Whether capture is running. */
  recording: boolean;
  /** Steps so far. */
  steps: RecordedStep[];
  /** Variables that hold passwords, saved masked. */
  secrets: string[];
}

/** Which recording request is in flight, if any. */
export type RecordBusy = 'start' | 'stop' | 'save' | null;

/** What GET /playbooks answers. */
export interface PlaybookList {
  /** Every saved playbook. */
  playbooks: PlaybookInfo[];
}

/** What POST /browsers/start answers. */
export interface StartedBrowser {
  /** The new browser's id. */
  id: string;
}

/** An API error that carries the server's machine-readable code. */
export interface CodedError {
  /** The response body. */
  body?: {
    /** Why, as a code: control_busy, commands_pending… */
    code?: string;
  };
}
