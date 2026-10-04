/**
 * What a caller is told about a native JavaScript dialog, and which dialogs are
 * answered for it. Shared by the desktop's dialog watcher (src/main/cdp/dialogs.ts)
 * and the server's CDP driver (server/src/drivers/dialogs.ts), so an agent
 * reads the same sentence whichever kind of browser it drives.
 *
 * An alert has one button, so answering it for the agent loses nothing as long
 * as the text is reported. A confirm or prompt is a decision ("delete this?"),
 * so it is held open and handed to the caller.
 */

/** A dialog as CDP's Page.javascriptDialogOpening reports it. */
export interface DialogInfo {
  /** alert, confirm, prompt or beforeunload. */
  type?: string;
  /** The text the page shows. */
  message?: string;
  /** A prompt's default answer. */
  defaultPrompt?: string;
}

/** Dialog types answered automatically: they have one outcome, so nothing is decided for the agent. */
export const AUTO_ACCEPT: ReadonlySet<string> = new Set(['alert', 'beforeunload']);

/**
 * How a note about a dialog still open ends. An action that answers with it did run:
 * it is what opened the dialog, so the recorder keeps it as a step.
 */
export const BLOCKED = 'The page is blocked until you call handle_dialog.';

/** The note a caller reads about a dialog, either answered for it or still waiting on handle_dialog. */
export function describe({ type, message, defaultPrompt }: DialogInfo = {}, handled = false): string {
  return handled
    ? `Dialog (${type}): "${message}", accepted automatically.`
    : `A JavaScript ${type} dialog is open: "${message}"${defaultPrompt ? ` (default: "${defaultPrompt}")` : ''}. ` +
        BLOCKED;
}
