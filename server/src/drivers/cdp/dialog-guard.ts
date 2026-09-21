/**
 * Native dialogs as the driver sees them: watching for them, holding a
 * confirm or prompt for the caller, and answering one command early when a
 * dialog blocks the page mid-command.
 */
import { AUTO_ACCEPT, describe as describeDialog } from '../dialogs.ts';
import type { CDPDriver } from './driver.ts';

/** What the dialog race settles with when a dialog is held open. */
const DIALOG_HELD = Symbol('dialog held');

/**
 * Dialogs are answered here, not inside dispatch(): a held confirm blocks the
 * renderer, so a command that touches the page would sit there until its
 * timeout and report nothing useful. One wrapper beats a guard in forty
 * returns.
 */
export async function sendGuarded(driver: CDPDriver, action, params, timeoutMs) {
  if (driver.pendingDialog && action !== 'handle_dialog')
    return { ok: false, error: describeDialog(driver.pendingDialog) };
  if (action === 'handle_dialog') return driver.answerDialog(params?.accept, params?.prompt_text ?? params?.promptText);
  // The dialog usually opens *during* the command that triggered it, a click
  // whose handler calls confirm() blocks the renderer before the click has
  // finished reporting. Racing the held dialog against the command answers the
  // caller now instead of waiting out a CDP timeout that cannot succeed.
  const result = await raceDialog(driver, action, params, timeoutMs);
  const note = driver.takeDialogNotes();
  if (result === DIALOG_HELD) return { ok: false, error: describeDialog(driver.pendingDialog) };
  return note ? { ...result, data: { ...(result?.data || {}), dialog: note } } : result;
}

/** Runs the command; answers with its result, or DIALOG_HELD if a dialog is held open first. */
async function raceDialog(driver: CDPDriver, action, params, timeoutMs) {
  const running = driver.dispatch(action, params, timeoutMs);
  running.catch(() => {}); // the loser of the race must not go unhandled
  const { held, release } = driver.dialogHeld();
  const result = await Promise.race([running, held]);
  release();
  return result;
}

/**
 * Settles when a dialog is held open. The caller must `release()` once it no
 * longer cares, every command asks, and a session that never sees a dialog
 * would otherwise pile up a resolver per command.
 */
export function dialogHeld(driver: CDPDriver) {
  let settle;
  const held = new Promise((resolve) => driver.dialogWaiters.push((settle = resolve)));
  const release = () => {
    driver.dialogWaiters = driver.dialogWaiters.filter((w) => w !== settle);
  };
  return { held, release };
}

/** Registers dialog listeners once: alerts and beforeunload are accepted, anything else is held for the caller. */
export function watchDialogs(driver: CDPDriver) {
  if (driver.dialogsWatched) return;
  Object.assign(driver, { dialogsWatched: true, dialogNotes: [], dialogWaiters: [] });
  driver.conn.on('Page.javascriptDialogClosed', (_params, sid) => {
    if (sid === driver.sessionId) driver.pendingDialog = null;
  });
  driver.conn.on('Page.javascriptDialogOpening', (params, sid) => onDialogOpening(driver, params, sid));
}

/** A dialog opened: accept it and note it, or hold it and wake whoever is racing a command against it. */
function onDialogOpening(driver: CDPDriver, params, sid) {
  if (sid !== driver.sessionId) return;
  const dialog = { type: params.type, message: params.message || '', defaultPrompt: params.defaultPrompt };
  if (!AUTO_ACCEPT.has(dialog.type)) return holdDialog(driver, dialog);
  driver.dialogNotes.push(describeDialog(dialog, true));
  driver.conn.send('Page.handleJavaScriptDialog', { accept: true }, sid).catch(() => {});
}

/** Keeps a confirm or prompt open for the caller to decide. */
function holdDialog(driver: CDPDriver, dialog) {
  driver.pendingDialog = dialog;
  driver.dialogNotes.push(describeDialog(dialog, false));
  for (const settle of driver.dialogWaiters.splice(0)) settle(DIALOG_HELD);
}

/** Returns and clears the dialog descriptions gathered during a command. */
export function takeDialogNotes(driver: CDPDriver) {
  if (!driver.dialogNotes?.length) return null;
  const notes = driver.dialogNotes.join(' ');
  driver.dialogNotes = [];
  return notes;
}

/** Answers the held dialog, with prompt text when it is a prompt. */
export async function answerDialog(driver: CDPDriver, accept, promptText) {
  const dialog = driver.pendingDialog;
  if (!dialog) return { ok: false, error: 'No dialog is open' };
  const args: any = { accept: accept !== false };
  if (promptText != null && dialog.type === 'prompt') args.promptText = String(promptText);
  driver.pendingDialog = null;
  await driver.conn.send('Page.handleJavaScriptDialog', args, driver.sessionId);
  return { ok: true, data: { type: dialog.type, message: dialog.message, accepted: args.accept } };
}
