/** Project playbook and replay shapes returned by the existing API. */
export interface Playbook {
  /** Saved name. */
  name: string;
  /** Replayable steps. */
  steps: number;
  /** Input names. */
  variables: string[];
  /** Recorded defaults. */
  defaults: Record<string, string>;
  /** Readable generated workflow code. */
  code: string;
  /**
   * How its last replay ended: null when it never ran, absent when the server
   * could not say (an older server, or its run history failed to load).
   */
  lastRun?: LastRun | null;
}
/** The end of a playbook's most recent replay, as the list reports it. */
export interface LastRun {
  /** How it ended. */
  status: 'succeeded' | 'failed';
  /** When it ended, as an ISO time. */
  endedAt: string;
}
/** State of a background replay, including human assistance. */
export interface PlaybookRun {
  /** Polling identifier. */
  id: string;
  /** Server lifecycle state. */
  status: string;
  /** Failure message. */
  error?: string;
  /** Request for human assistance. */
  attention?: { /** What the person needs to do. */ message: string } | null;
  /** Completed result. */
  result?: Record<string, unknown>;
}
