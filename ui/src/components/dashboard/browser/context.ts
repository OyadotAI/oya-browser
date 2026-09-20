/**
 * What the panel's action hooks share: the browser, the key, the input sender,
 * and the one busy slot that says which action is running.
 */
import type { useToast } from '../toast';
import type { BrowserDetail } from '../types';
import type { Busy, Send } from './types';

/** The panel's shared state and callbacks, handed to each action hook. */
export interface PanelContext {
  /** The browser the panel shows. */
  browserId: string;
  /** The project key. */
  apiKey: string;
  /** Latest detail, or null before the first poll. */
  detail: BrowserDetail | null;
  /** Sends one input command. */
  send: Send;
  /** Adds an optimistic activity line. */
  onInput: (line: string) => void;
  /** Polls the detail now. */
  refresh: () => void;
  /** Marks which action is running; null when none. */
  setBusy: (b: Busy) => void;
  /** Shows a toast. */
  toast: ReturnType<typeof useToast>;
}

/** Runs `fn` with the busy slot set to `name`, then clears it. */
export async function withBusy<T>(setBusy: (b: Busy) => void, name: string, fn: () => Promise<T>): Promise<T> {
  setBusy(name);
  const result = await fn();
  setBusy(null);
  return result;
}
