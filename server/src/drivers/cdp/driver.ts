/**
 * The CDP driver: one browser, driven with the same action vocabulary as the
 * Oya client. Each concern lives in its own file; this class is the surface
 * the rest of the server calls.
 */
import { CDPDriverState } from './state.ts';
import { connectDriver, attachTarget } from './session.ts';
import { collectRecording, armRecording } from './recording.ts';
import { ensureWorld, evaluateInWorld, evaluateInMain } from './world.ts';
import { sendGuarded, dialogHeld, watchDialogs, takeDialogNotes, answerDialog } from './dialog-guard.ts';
import { dispatch } from './handlers/index.ts';
import { startScreencast, stopScreencast } from './screencast.ts';
import { FIND_ELEMENT_JS } from './page-scripts.ts';
import { COMMAND_TIMEOUT_MS, FALLBACK_VIEWPORT } from './constants.ts';

/** Drives one CDP browser with the same action vocabulary as the Oya client. */
export class CDPDriver extends CDPDriverState {
  /** Connects, attaches to a page (creating one if none), and restores saved cookies. */
  async connect() {
    return connectDriver(this);
  }

  /** Attaches to a target and prepares it: domains, dialogs, persona, recording and login state. */
  async attach(targetId) {
    return attachTarget(this, targetId);
  }

  /** Adds newly recorded steps and secret names from the page, skipping duplicates. */
  collectRecording(out) {
    collectRecording(this, out);
  }

  /** Starts streaming recorded steps from the current target. */
  async armRecording() {
    return armRecording(this);
  }

  /** Whether the CDP connection is still open. */
  isAlive() {
    return !!this.conn && !this.conn.closed;
  }

  /** The isolated world the analyzer runs in, created (with the analyzer in it) when missing or forced. */
  async ensureWorld({ force = false } = {}) {
    return ensureWorld(this, { force });
  }

  /** Everything the analyzer needs runs here, never in the page's own world. */
  async evaluate(expression, { awaitPromise = true, retry = true } = {}) {
    return evaluateInWorld(this, expression, { awaitPromise, retry });
  }

  /** The world is created with the analyzer already in it. */
  async ensureAnalyzer() {
    await this.ensureWorld();
  }

  /** Evaluates in the page's own world, for scripts that must reach page globals (CAPTCHA, MFA). */
  async evaluateMain(expression, { awaitPromise = true } = {}) {
    return evaluateInMain(this, expression, { awaitPromise });
  }

  /** Dispatches one mouse event to the page. */
  async mouse(type, x, y, button = 'left', clickCount = 1) {
    await this.conn.send('Input.dispatchMouseEvent', { type, x, y, button, clickCount }, this.sessionId);
  }

  /** A left click at viewport coordinates. */
  async clickAt(x, y) {
    await this.mouse('mousePressed', x, y);
    await this.mouse('mouseReleased', x, y);
  }

  /** Scrolls an element into view and returns its centre point. */
  async locate(selector) {
    await this.ensureAnalyzer();
    const found = await this.evaluate(FIND_ELEMENT_JS(selector));
    if (!found?.ok) throw new Error(found?.error || 'Element not found');
    return found.data;
  }

  /** The viewport size, falling back to 1280×800. */
  async viewport() {
    return (await this.evaluate('({width: innerWidth, height: innerHeight})')) || { ...FALLBACK_VIEWPORT };
  }

  /** Runs one action, answering early when a dialog blocks the page; see sendGuarded. */
  async send(action, params: any = {}, timeoutMs = COMMAND_TIMEOUT_MS) {
    return sendGuarded(this, action, params, timeoutMs);
  }

  /** Settles when a dialog is held open; the caller must `release()` once it no longer cares. */
  dialogHeld() {
    return dialogHeld(this);
  }

  /** Registers dialog listeners once: alerts and beforeunload are accepted, anything else is held for the caller. */
  watchDialogs() {
    watchDialogs(this);
  }

  /** Returns and clears the dialog descriptions gathered during a command. */
  takeDialogNotes() {
    return takeDialogNotes(this);
  }

  /** Answers the held dialog, with prompt text when it is a prompt. */
  async answerDialog(accept, promptText) {
    return answerDialog(this, accept, promptText);
  }

  /** Runs one action through the command map, in either spelling of the vocabulary. */
  async dispatch(action, params: any = {}, timeoutMs = COMMAND_TIMEOUT_MS) {
    return dispatch(this, action, params, timeoutMs);
  }

  /** Streams live-view frames to `onFrame`, filling idle gaps with screenshots. */
  async startScreencast(onFrame, options = {}) {
    return startScreencast(this, onFrame, options);
  }

  /** Stops the live-view screencast and its idle fill. */
  async stopScreencast() {
    return stopScreencast(this);
  }

  /** The current page's URL and title, or blanks when the page cannot be read. */
  async pageInfo() {
    const info = await this.evaluate('({ url: location.href, title: document.title })').catch(() => null);
    return info || { url: '', title: '' };
  }

  /** Closes the connection and stops the idle-fill timer. */
  close() {
    clearInterval(this.screencastFill);
    this.conn?.close();
  }
}
