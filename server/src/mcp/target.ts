/**
 * Where an MCP tool call goes, shared by the direct tools (browser-tools.ts) and
 * the agent's tools (agent-tools.ts) so both answer a missing or starting browser
 * in the same words: the picked browser, or a connector's own browser acquired on
 * demand, with a note when it was just started.
 */
import { sendCommand } from '../modules/browsers/socket.ts';
import { attempt } from './replies.ts';
import { DIALOG_DISMISS_MS } from './constants.ts';

/** A browser to send the call to. */
export interface Found {
  /** The browser. */
  id: string;
  /** Said before the tool's own reply, e.g. that a browser was just started. */
  note?: string;
}

/** Why there is no browser to send the call to. */
export interface Missing {
  /** What the model is told instead. */
  error: string;
}

/** A browser for the call, or why there is none. */
export type Acquired = Found | Missing;

/** How a server finds its browser: picked from the pool, or acquired for a connector. */
export interface Targeting {
  /** The pool's pick, or null when it has none. */
  pick: () => string | null | undefined;
  /**
   * A connector's acquisition; when set, it is used instead of pick. `start` says whether this
   * call may start a browser: only the tools that open a page do, so a read-only tool (which a
   * chat app may run unasked) never begins a paid session.
   */
  acquire?: (start: boolean) => Promise<Acquired>;
  /** Error text when pick finds no browser. */
  noBrowser: string;
}

/** The browser a call goes to, or why there is none; `opens` says whether the call opens a page, and so may start a connector's browser. */
export async function targetOf(opens: boolean, { pick, acquire, noBrowser }: Targeting): Promise<Acquired> {
  if (acquire) return acquire(opens);
  const id = pick();
  return id ? { id } : { error: noBrowser };
}

/** What a connector is told after a held dialog was cancelled for it. */
const DISMISSED =
  'Oya dismissed it with Cancel. If the step did not happen, repeat it; if the person needed to accept it, tell them.';

/**
 * A chat connector cannot answer dialogs (it has no handle_dialog), so after a failed call
 * the dialog that may be holding the page is cancelled: Cancel never lets an action go
 * ahead. With none open the browser refuses, and the original error stands.
 */
export async function dismissDialog(id: string, error: string) {
  const answer = await attempt(() => sendCommand(id, 'handle_dialog', { accept: false }, DIALOG_DISMISS_MS));
  return 'value' in answer && answer.value.ok ? `${error}\n${DISMISSED}` : error;
}

/** The reply's text with the acquisition's note before it. */
export const noted = (note: string | undefined, text: string) => (note ? `${note}\n${text}` : text);
