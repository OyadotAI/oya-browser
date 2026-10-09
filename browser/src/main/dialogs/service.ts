/** Dialog decisions are queued per source, independent of any debugging transport. */
import { describe } from '../../page/dialog-text.ts';
import { DIALOG_HELD } from './constants.ts';
import type { Dialog, DialogAnswer, DialogText, DialogWait } from './types.ts';
/** Shared wording remains stable for existing clients. */
export const describeDialog: (dialog: DialogText, handled?: boolean) => string = describe;
/** A command waiting for a decision boundary. */
type Waiter = (value: typeof DIALOG_HELD) => void;
/** Retains simultaneous dialogs instead of overwriting one tab's decision with another's. */
export class DialogService {
  /** Informational alerts are also delivered to the browser notification inbox. */
  private readonly notify: (message: string, url?: string) => void;
  /** Arrival order is preserved across independent page surfaces. */
  private pending: Dialog[] = [];
  /** Each note is delivered to the agent once. */
  private notes: string[] = [];
  /** Every interrupted command has an individually releasable waiter. */
  private waiters = new Set<Waiter>();
  /** No debugger, protocol sender or remote endpoint enters the decision service. */
  constructor(notify: (message: string, url?: string) => void = () => {}) {
    this.notify = notify;
  }
  /** Already-held dialogs wake new callers immediately instead of leaving them waiting forever. */
  held(): DialogWait {
    const { promise: held, resolve } = Promise.withResolvers<typeof DIALOG_HELD>();
    if (this.pending.length) resolve(DIALOG_HELD);
    else this.waiters.add(resolve);
    return { held, release: () => this.waiters.delete(resolve) };
  }

  /** Drain notes atomically so concurrent results do not report an alert twice. */
  takeNotes(): string | null {
    if (!this.notes.length) return null;
    const notes = this.notes.join(' ');
    this.notes = [];
    return notes;
  }
  /** Only the oldest live decision may be answered by the existing command contract. */
  async answer(accept?: boolean, promptText?: unknown): Promise<DialogAnswer> {
    const dialog = this.pending.shift();
    if (!dialog) return { ok: false, error: 'No dialog is open' };
    const { type, message, reply } = dialog;
    const text = promptText != null && type === 'prompt' ? String(promptText) : undefined;
    await reply(accept !== false, text);
    return { ok: true, data: { type, message, accepted: accept !== false } };
  }
  /** Inspect the next held decision without consuming it. */
  current(): Dialog | null {
    return this.pending[0] ?? null;
  }
  /** Cancellation belongs to one source, never every tab's pending decisions. */
  cancel(source: object): void {
    this.pending = this.pending.filter((dialog) => dialog.source !== source);
  }
  /** Alerts are informational; all other native decisions remain explicit. */
  receive(dialog: Dialog, url?: string, automatic = dialog.type === 'alert'): void {
    this.notes.push(describeDialog(dialog, automatic));
    if (automatic) this.accept(dialog);
    else this.hold(dialog);
    if (dialog.type === 'alert') this.notify(dialog.message, url);
  }
  /** Wake interrupted commands only after the dialog is visible through current(). */
  private hold(dialog: Dialog): void {
    this.pending.push(dialog);
    for (const settle of this.waiters) settle(DIALOG_HELD);
    this.waiters.clear();
  }
  /** Report reply failures without converting a confirmation into acceptance. */
  private async accept(dialog: Dialog): Promise<void> {
    try {
      await dialog.reply(true);
    } catch (error) {
      console.error('[oya] could not answer dialog:', error);
    }
  }
}
