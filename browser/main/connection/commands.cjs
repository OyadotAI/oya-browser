/**
 * Commands from the server (`cmd` messages): run each one, answer it once, and
 * race it against a JavaScript dialog that would otherwise block it forever.
 */
const {
  DIALOG_SAFE_ACTIONS,
  DIALOG_HELD,
  dialogHeld,
  describeDialog,
  takeDialogNotes,
  answerDialog,
  currentDialog,
} = require('../dialogs.cjs');
const { resultSummary } = require('./result-summary.cjs');
const { TAB_COMMANDS } = require('./tab-commands.cjs');
const { RESULT_CODES } = require('./constants.cjs');

/** The code to send with a failed result: one of ours, or none. */
const codeOf = (err) => (RESULT_CODES.has(err?.code) ? err.code : undefined);

/** Runs server commands and sends their results. */
class CommandRunner {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /**
     * Commands already answered early because a dialog interrupted them. Each entry
     * is removed by the late answer it is waiting for.
     * ponytail: an entry outlives the session if that answer never lands; bounded by
     * the number of dialogs raised.
     */
    this.answeredCommands = new Set();
  }

  /**
   * A confirm() or prompt() opens *during* the command that triggered it and
   * blocks the renderer, so the command it interrupted can never finish. Race the
   * held dialog against it: the caller hears about the dialog now instead of
   * waiting out a timeout that cannot succeed. The interrupted command is left to
   * settle on its own; sendResult drops its late answer.
   */
  async handleCommand(msg) {
    const { held, release } = dialogHeld();
    const ran = this.runSafely(msg);
    const outcome = await Promise.race([ran, held]);
    release();
    if (outcome !== DIALOG_HELD) return;
    // Answer first: marking the id before this would make sendResult drop the very
    // result being sent, and the caller would wait out the timeout after all.
    this.sendResult(msg.id, false, null, describeDialog(currentDialog(), false));
    // The interrupted command is still out there; drop its answer when it lands.
    this.answeredCommands.add(msg.id);
  }

  /**
   * handleCommand is called fire-and-forget by the socket router, so a throw
   * escaping here would take down the main process rather than one command.
   */
  runSafely(msg) {
    const failed = (e) => {
      this.sendResult(msg.id, false, null, e?.message || String(e), codeOf(e));
      return null;
    };
    return this.runCommand(msg).then(() => null, failed);
  }

  /** Answers one command. */
  async runCommand(msg) {
    const { id, action, params } = msg;
    if (!this.ctx.shell.browsingMode) return this.sendResult(id, false, null, 'Browser not ready');
    // The command maps look an action up as a key, which would read `["evaluate_raw"]` as the string.
    if (typeof action !== 'string') return this.sendResult(id, false, null, 'action must be a string');
    // A held dialog blocks the renderer: anything that touches the page would sit
    // there until its timeout and tell the caller nothing. Answer with the dialog
    // instead, so the next move is obvious and costs no wall clock.
    if (this.blockedByDialog(id, action)) return;
    if (action === 'handle_dialog') return this.handleDialog(id, params);
    await this.dispatch(id, action, params).catch((err) =>
      this.sendResult(id, false, null, err.message || String(err), codeOf(err)),
    );
  }

  /** Answers with the held dialog when it blocks this action. */
  blockedByDialog(id, action) {
    if (!currentDialog() || DIALOG_SAFE_ACTIONS.has(action)) return false;
    this.sendResult(id, false, null, describeDialog(currentDialog(), false));
    return true;
  }

  /** Accepts or dismisses the open dialog. */
  async handleDialog(id, params) {
    const r = await answerDialog(params?.accept, params?.prompt_text ?? params?.promptText);
    this.sendResult(id, r.ok, r.data, r.error);
  }

  /** Tab-level commands here; everything else acts on the active page. */
  async dispatch(id, action, params) {
    if (Object.hasOwn(TAB_COMMANDS, action)) return TAB_COMMANDS[action](this, id, params);
    // All remaining actions need an active tab
    const view = this.ctx.tabs.getActiveView();
    if (!view || view.webContents.isDestroyed()) return this.sendResult(id, false, null, 'No active tab');
    await this.ctx.actions.runPageAction(id, action, params, view);
  }

  /** Sends one result, with any dialog that fired during it, and its code when it has one of ours; drops a late answer already given. */
  sendResult(id, ok, data, error, code) {
    if (!this.ctx.socket.isOpen()) return;
    if (this.answeredCommands.delete(id)) return;
    // Every command result passes through here, which makes it the one place a
    // dialog that fired mid-action can be reported without editing 40 call sites.
    const dialog = takeDialogNotes();
    if (dialog) data = { ...(data || {}), dialog };
    this.ctx.shell.devLog('out', ok ? 'result: ok' : 'result: error', resultSummary(id, ok, data, error));
    const coded = RESULT_CODES.has(code) ? { code } : {};
    this.ctx.socket.send({ type: 'cmd_result', id, ok, data: data || null, error: error || null, ...coded });
  }
}

module.exports = { CommandRunner };
