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
const AUTO_ACCEPT_DIALOGS = new Set(['alert', 'beforeunload']);
// Answerable while a dialog is held: they never reach the blocked renderer.
const DIALOG_SAFE_ACTIONS = new Set([
  'handle_dialog',
  'screenshot',
  'list_tabs',
  'read_console',
  'read_network',
  'record',
  'workflow',
]);
// ponytail: one pending dialog across all tabs, a blocked tab cannot raise a
// second one, and only the active tab is driven. Per-view if that stops holding.
let pendingDialog = null;
let dialogNotes = [];
let dialogWaiters = [];

/**
 * Settles when a dialog is held open. The caller must call the returned
 * `release` once it no longer cares, every command asks, and a session that
 * never sees a dialog would otherwise pile up a resolver per command.
 */
const DIALOG_HELD = Symbol('dialog held');
/** A promise that settles with DIALOG_HELD when a dialog is held, and its `release`. */
function dialogHeld() {
  let settle;
  const held = new Promise((resolve) => {
    settle = resolve;
    dialogWaiters.push(resolve);
  });
  return { held, release: () => dropDialogWaiter(settle) };
}

/** Stops waking `settle` when a dialog is held. */
function dropDialogWaiter(settle) {
  dialogWaiters = dialogWaiters.filter((w) => w !== settle);
}

/** The sentence the driver reads about a dialog: answered automatically, or waiting on handle_dialog. */
function describeDialog({ type, message, defaultPrompt } = {}, handled = false) {
  return handled
    ? `Dialog (${type}): "${message}", accepted automatically.`
    : `A JavaScript ${type} dialog is open: "${message}"${defaultPrompt ? ` (default: "${defaultPrompt}")` : ''}. ` +
        'The page is blocked until you call handle_dialog.';
}

/** Every dialog note since the last call, as one string, or null when there were none. */
function takeDialogNotes() {
  if (!dialogNotes.length) return null;
  const notes = dialogNotes.join(' ');
  dialogNotes = [];
  return notes;
}

/** Answer whatever dialog is open. Safe to call when none is. */
async function answerDialog(accept, promptText) {
  if (!pendingDialog) return { ok: false, error: 'No dialog is open' };
  const { dbg, type, message } = pendingDialog;
  const params = { accept: accept !== false };
  if (promptText != null && type === 'prompt') params.promptText = String(promptText);
  pendingDialog = null;
  await dbg.sendCommand('Page.handleJavaScriptDialog', params);
  return { ok: true, data: { type, message, accepted: params.accept } };
}

/** A confirm or prompt: keep it open for the driver and wake every command waiting on one. */
function holdDialog(dialog) {
  pendingDialog = dialog;
  dialogNotes.push(describeDialog(dialog, false));
  for (const settle of dialogWaiters.splice(0)) settle(DIALOG_HELD);
}

/** An alert or beforeunload: note it and click its only button. */
function acceptDialog(dialog) {
  dialogNotes.push(describeDialog(dialog, true));
  dialog.dbg
    .sendCommand('Page.handleJavaScriptDialog', { accept: true })
    .catch((e) => console.error('[oya] could not answer dialog:', e?.message || e));
}

/** One CDP event from a watched debugger. */
function onDialogEvent(dbg, method, params) {
  if (method === 'Page.javascriptDialogClosed') {
    pendingDialog = null;
    return;
  }
  if (method !== 'Page.javascriptDialogOpening') return;
  const dialog = { dbg, type: params.type, message: params.message || '', defaultPrompt: params.defaultPrompt };
  if (AUTO_ACCEPT_DIALOGS.has(dialog.type)) acceptDialog(dialog);
  else holdDialog(dialog);
}

/**
 * Watch one debugger session for dialogs. Called for every tab and every popup;
 * without it that surface wedges on the first alert().
 */
function attachDialogWatcher(dbg) {
  if (!dbg || dbg.oyaDialogWatcher) return;
  dbg.oyaDialogWatcher = true;
  dbg.on('message', (_event, method, params) => onDialogEvent(dbg, method, params));
}

/** The confirm or prompt currently held open, if any. */
const currentDialog = () => pendingDialog;

module.exports = {
  AUTO_ACCEPT_DIALOGS,
  DIALOG_SAFE_ACTIONS,
  DIALOG_HELD,
  dialogHeld,
  describeDialog,
  takeDialogNotes,
  answerDialog,
  attachDialogWatcher,
  currentDialog,
};
