/**
 * Chrome DevTools Protocol on an Electron view's own debugger: attach on first
 * use, send a command, evaluate an expression.
 */
import type { Debugger, WebContents } from 'electron';
import { CDP_VERSION, DEBUGGER_MAX_LISTENERS } from './constants.ts';

export { CDP_VERSION };

/** Anything that shows a page: a tab's view, the shell window, a hidden window. */
export interface PageView {
  /** The page's contents, whose debugger CDP rides on. */
  readonly webContents: WebContents;
}

/** Attaches a debugger; an attach that fails is left for the command to report. */
function attachQuietly(dbg: Debugger): void {
  try {
    dbg.attach(CDP_VERSION);
  } catch {}
}

/** The view's debugger, attached; null once the view is gone. */
export function cdpAttach(view: PageView | null | undefined): Debugger | null {
  if (!view || view.webContents.isDestroyed()) return null;
  const dbg = view.webContents.debugger;
  dbg.setMaxListeners?.(DEBUGGER_MAX_LISTENERS);
  if (!dbg.isAttached()) attachQuietly(dbg);
  return dbg;
}

/** Sends one CDP command to the view and answers its result. */
export async function cdp<T = unknown>(
  view: PageView | null | undefined,
  method: string,
  params: object = {},
): Promise<T> {
  const dbg = cdpAttach(view);
  if (!dbg) throw new Error('View is destroyed');
  return dbg.sendCommand(method, params);
}

/** A value Runtime.evaluate returned by value. */
interface RemoteValue {
  /** The value itself. */
  value?: unknown;
}

/** Why an evaluation threw. */
interface ExceptionDetails {
  /** The exception's message. */
  text?: string;
}

/** What Runtime.evaluate answers. */
interface Evaluation {
  /** The value, when the expression returned one. */
  result?: RemoteValue;
  /** Why it threw, when it threw. */
  exceptionDetails?: ExceptionDetails;
}

/** Evaluates `expression` in the page's main world and returns its value; a thrown exception rejects. */
export async function cdpEval<T = unknown>(view: PageView, expression: string): Promise<T> {
  const params = { expression, returnByValue: true, awaitPromise: true };
  const res = await cdp<Evaluation>(view, 'Runtime.evaluate', params);
  if (res.exceptionDetails) throw new Error(res.exceptionDetails.text || 'Eval failed');
  return res.result?.value as T;
}
