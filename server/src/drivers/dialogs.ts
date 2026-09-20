/**
 * Native JavaScript dialogs (alert / confirm / prompt / beforeunload).
 *
 * With the CDP Page domain enabled and nobody answering
 * Page.javascriptDialogOpening, Chromium hands the dialog to the attached client
 * and the renderer blocks forever — every later command then eats its full
 * timeout and the run dies with no clue why. Puppeteer and Playwright both ship
 * an auto-dismiss for exactly this reason.
 *
 * An alert has one button, so answering it for the agent loses nothing as long
 * as the text is reported. A confirm or prompt is a decision ("delete this?"),
 * so it is held open and handed to the model.
 */

/** Dialog types answered automatically: they have one outcome, so nothing is decided for the agent. */
export const AUTO_ACCEPT = new Set(['alert', 'beforeunload']);

/** The note the model reads about a dialog, either answered for it or still waiting on handle_dialog. */
export const describe = ({ type, message, defaultPrompt }: any = {}, handled = false) =>
  handled
    ? `Dialog (${type}): "${message}" — accepted automatically.`
    : `A JavaScript ${type} dialog is open: "${message}"${defaultPrompt ? ` (default: "${defaultPrompt}")` : ''}. ` +
      'The page is blocked until you call handle_dialog.';
