/**
 * Shapes the browser panel passes between its hooks and parts.
 */

/** One page element from `analyze`. */
export interface PageElement {
  /** The id `click` takes. */
  id: number;
  /** Element kind: link, button, input… */
  type: string;
  /** Visible text, if any. */
  text?: string;
  /** Whether it is on screen; only visible ones are listed. */
  visible: boolean;
}

/** What a control-session input answers. */
export interface InputResult {
  /** False when the command failed. */
  ok: boolean;
  /** The command's result, e.g. a screenshot or elements. */
  data?: unknown;
  /** Why it failed. */
  error?: string;
}

/** Sends one input command to the browser; toasts failures. */
export type Send = (action: string, params?: Record<string, unknown>) => Promise<InputResult>;

/** A line shown in the activity log until the server echoes it. */
export interface OptimisticLine {
  /** When the user did it. */
  ts: string;
  /** What they did, in one line. */
  line: string;
}

/** Which panel action is running, if any; one at a time. */
export type Busy = string | null;
