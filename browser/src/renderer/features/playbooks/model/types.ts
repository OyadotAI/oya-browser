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
