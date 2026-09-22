/**
 * What a caller is told about a native JavaScript dialog, and which dialogs are
 * answered for it. Shared by the desktop's dialog watcher (main/dialogs.cjs)
 * and the server's CDP driver (server/src/drivers/dialogs.ts), so an agent
 * reads the same sentence whichever kind of browser it drives.
 *
 * An alert has one button, so answering it for the agent loses nothing as long
 * as the text is reported. A confirm or prompt is a decision ("delete this?"),
 * so it is held open and handed to the caller.
 */

/** Dialog types answered automatically: they have one outcome, so nothing is decided for the agent. */
const AUTO_ACCEPT = new Set(['alert', 'beforeunload']);

/** The note a caller reads about a dialog, either answered for it or still waiting on handle_dialog. */
function describe({ type, message, defaultPrompt } = {}, handled = false) {
  return handled
    ? `Dialog (${type}): "${message}", accepted automatically.`
    : `A JavaScript ${type} dialog is open: "${message}"${defaultPrompt ? ` (default: "${defaultPrompt}")` : ''}. ` +
        'The page is blocked until you call handle_dialog.';
}

module.exports = { AUTO_ACCEPT, describe };
