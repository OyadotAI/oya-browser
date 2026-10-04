/**
 * Native JavaScript dialogs.
 *
 * Page.enable is on for every tab and popup below. With the Page domain enabled
 * and nobody answering Page.javascriptDialogOpening, Chromium hands the dialog
 * to us and the renderer blocks forever: every later command then eats its full
 * 60s timeout and the agent is told nothing about why. Puppeteer and Playwright
 * both ship an auto-dismiss for exactly this reason.
 *
 * An alert has one button, so answering it costs nothing as long as its text is
 * reported back. A confirm or prompt is a decision ("delete this?"), so it is
 * held open and handed to whoever is driving.
 */
import type { Debugger } from 'electron';
import { AUTO_ACCEPT, describe } from '../../page/dialog-text.ts';
import { DIALOG_SAFE_ACTIONS } from './constants.ts';

export { DIALOG_SAFE_ACTIONS };

/** Dialog types answered automatically; the one set the server's CDP driver uses too. */
export const AUTO_ACCEPT_DIALOGS: ReadonlySet<string> = AUTO_ACCEPT;

/** The sentence the driver reads about a dialog: answered automatically, or waiting on handle_dialog. Shared with the server. */
export const describeDialog: (dialog: DialogText, handled?: boolean) => string = describe;

/** What a dialog held() promise settles with. */
export const DIALOG_HELD = Symbol('dialog held');

/** The debugger a dialog came from: enough to answer it and to hear the next one. */
export type DialogDebugger = Pick<Debugger, 'on' | 'sendCommand'> & DialogMark;

/** The mark a watched debugger carries, so a surface is watched once and its setup can be checked. */
export interface DialogMark {
  /** Set once a dialog watcher listens on this debugger. */
  oyaDialogWatcher?: boolean;
}

/** What the sentence about a dialog is made of. */
export interface DialogText {
  /** alert, confirm, prompt or beforeunload. */
  type: string;
  /** The text the page showed. */
  message: string;
  /** A prompt's prefilled answer. */
  defaultPrompt?: string;
}

/** A dialog the page raised, and the debugger to answer it on. */
export interface Dialog extends DialogText {
  /** The debugger of the surface that raised it. */
  dbg: DialogDebugger;
}

/** A wait for a held dialog, and how to stop waiting. */
export interface DialogWait {
  /** Settles with DIALOG_HELD when a dialog is held. */
  held: Promise<typeof DIALOG_HELD>;
  /** Stops waiting; call it once the answer no longer matters. */
  release: () => void;
}

/** What answer() reports. */
export interface DialogAnswer {
  /** Whether a dialog was open to answer. */
  ok: boolean;
  /** Why not, when none was. */
  error?: string;
  /** What was answered. */
  data?: AnsweredDialog;
}

/** A dialog answer() closed, and how. */
export interface AnsweredDialog {
  /** The dialog's type. */
  type: string;
  /** Its text. */
  message: string;
  /** Whether it was accepted rather than dismissed. */
  accepted: boolean;
}

/** Page.handleJavaScriptDialog's parameters. */
interface DialogReply {
  /** Accept rather than dismiss. */
  accept: boolean;
  /** A prompt's answer. */
  promptText?: string;
}

/** What Page.javascriptDialogOpening carries. */
interface OpeningParams {
  /** The dialog's type. */
  type: string;
  /** Its text. */
  message?: string;
  /** A prompt's prefilled answer. */
  defaultPrompt?: string;
}

/** Wakes a command waiting on a held dialog. */
type Waiter = (value: typeof DIALOG_HELD) => void;

/**
 * The dialogs of every watched surface: alerts answered and noted, a confirm or
 * prompt held for the driver.
 * ponytail: one pending dialog across all tabs, a blocked tab cannot raise a
 * second one, and only the active tab is driven. Per-view if that stops holding.
 */
export class Dialogs {
  /** The confirm or prompt held open, if any. */
  private pending: Dialog | null = null;
  /** Sentences about dialogs since the last takeNotes(). */
  private notes: string[] = [];
  /** Commands waiting to hear a dialog is held. */
  private waiters: Waiter[] = [];
  /**
   * Settles when a dialog is held open. The caller must call the returned
   * `release` once it no longer cares, every command asks, and a session that
   * never sees a dialog would otherwise pile up a resolver per command.
   */
  held(): DialogWait {
    let settle: Waiter = () => {};
    const held = new Promise<typeof DIALOG_HELD>((resolve) => {
      settle = resolve;
      this.waiters.push(resolve);
    });
    return { held, release: () => this.drop(settle) };
  }

  /** Stops waking `settle` when a dialog is held. */
  private drop(settle: Waiter): void {
    this.waiters = this.waiters.filter((w) => w !== settle);
  }

  /** Every dialog note since the last call, as one string, or null when there were none. */
  takeNotes(): string | null {
    if (!this.notes.length) return null;
    const notes = this.notes.join(' ');
    this.notes = [];
    return notes;
  }

  /** Answer whatever dialog is open. Safe to call when none is. */
  async answer(accept?: boolean, promptText?: unknown): Promise<DialogAnswer> {
    if (!this.pending) return { ok: false, error: 'No dialog is open' };
    const { dbg, type, message } = this.pending;
    const params: DialogReply = { accept: accept !== false };
    if (promptText != null && type === 'prompt') params.promptText = String(promptText);
    this.pending = null;
    await dbg.sendCommand('Page.handleJavaScriptDialog', params);
    return { ok: true, data: { type, message, accepted: params.accept } };
  }

  /** The confirm or prompt currently held open, if any. */
  current(): Dialog | null {
    return this.pending;
  }

  /**
   * Watch one debugger session for dialogs. Called for every tab and every popup;
   * without it that surface wedges on the first alert().
   */
  watch(dbg: DialogDebugger | null | undefined): void {
    if (!dbg || dbg.oyaDialogWatcher) return;
    dbg.oyaDialogWatcher = true;
    dbg.on('message', (_event, method, params) => this.onEvent(dbg, method, params));
  }

  /** One CDP event from a watched debugger. */
  private onEvent(dbg: DialogDebugger, method: string, params: OpeningParams): void {
    if (method === 'Page.javascriptDialogClosed') {
      this.pending = null;
      return;
    }
    if (method !== 'Page.javascriptDialogOpening') return;
    const dialog = { dbg, type: params.type, message: params.message || '', defaultPrompt: params.defaultPrompt };
    if (AUTO_ACCEPT_DIALOGS.has(dialog.type)) this.accept(dialog);
    else this.hold(dialog);
  }

  /** A confirm or prompt: keep it open for the driver and wake every command waiting on one. */
  private hold(dialog: Dialog): void {
    this.pending = dialog;
    this.notes.push(describeDialog(dialog, false));
    for (const settle of this.waiters.splice(0)) settle(DIALOG_HELD);
  }

  /** An alert or beforeunload: note it and click its only button. */
  private accept(dialog: Dialog): void {
    this.notes.push(describeDialog(dialog, true));
    dialog.dbg
      .sendCommand('Page.handleJavaScriptDialog', { accept: true })
      .catch((e: unknown) => console.error('[oya] could not answer dialog:', messageOrSelf(e)));
  }
}

/** An error's message, or the thrown value itself when it has none. */
const messageOrSelf = (e: unknown): unknown => (e as Error | null)?.message || e;
