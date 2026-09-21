/**
 * Unit tests for main/dialogs.cjs: alerts are answered at once and reported,
 * a confirm or prompt is held for the driver, and every surface is watched once.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { FakeDebugger, freshRequire, flush } = require('../support/fakes.cjs');

describe('dialogs', () => {
  let dialogs;
  let dbg;
  beforeEach(() => {
    dialogs = freshRequire('main/dialogs.cjs');
    dbg = new FakeDebugger();
    dialogs.attachDialogWatcher(dbg);
  });

  it('accepts an alert at once and notes its text for the next result', () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'alert', message: 'Saved' });
    assert.deepEqual(dbg.sent[0], {
      method: 'Page.handleJavaScriptDialog',
      params: { accept: true },
      sessionId: undefined,
    });
    assert.equal(dialogs.takeDialogNotes(), 'Dialog (alert): "Saved", accepted automatically.');
    assert.equal(dialogs.takeDialogNotes(), null, 'notes are taken once');
    assert.equal(dialogs.currentDialog(), null);
  });

  it('holds a confirm open and wakes every command waiting on a dialog', async () => {
    const { held } = dialogs.dialogHeld();
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'Delete?' });
    assert.equal(await held, dialogs.DIALOG_HELD);
    assert.equal(dialogs.currentDialog().type, 'confirm');
    assert.equal(dbg.sent.length, 0, 'a decision is not answered automatically');
  });

  it('names the default of a held prompt and says how to unblock it', () => {
    const text = dialogs.describeDialog({ type: 'prompt', message: 'Name?', defaultPrompt: 'Ann' });
    assert.equal(
      text,
      'A JavaScript prompt dialog is open: "Name?" (default: "Ann"). The page is blocked until you call handle_dialog.',
    );
  });

  it('does not wake a waiter that was released', async () => {
    const { held, release } = dialogs.dialogHeld();
    release();
    let woke = false;
    held.then(() => (woke = true));
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'x' });
    await flush();
    assert.equal(woke, false);
  });

  it('answers the held prompt with the given text and clears it', async () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'prompt', message: 'Name?' });
    const answer = await dialogs.answerDialog(true, 42);
    assert.deepEqual(dbg.sent.at(-1).params, { accept: true, promptText: '42' });
    assert.deepEqual(answer, { ok: true, data: { type: 'prompt', message: 'Name?', accepted: true } });
    assert.equal(dialogs.currentDialog(), null);
  });

  it('dismisses when accept is false and sends no text for a confirm', async () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'Sure?' });
    await dialogs.answerDialog(false, 'ignored');
    assert.deepEqual(dbg.sent.at(-1).params, { accept: false });
  });

  it('reports that no dialog is open when there is none to answer', async () => {
    assert.deepEqual(await dialogs.answerDialog(true), { ok: false, error: 'No dialog is open' });
  });

  it('forgets the held dialog once the page closes it', () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'x' });
    dbg.event('Page.javascriptDialogClosed');
    assert.equal(dialogs.currentDialog(), null);
  });

  it('watches a debugger once however often it is attached', () => {
    dialogs.attachDialogWatcher(dbg);
    dialogs.attachDialogWatcher(null);
    dbg.event('Page.javascriptDialogOpening', { type: 'alert', message: 'once' });
    assert.equal(dbg.sent.length, 1);
  });

  it('keeps screenshots and tab listing answerable while a dialog blocks the page', () => {
    assert.ok(dialogs.DIALOG_SAFE_ACTIONS.has('screenshot'));
    assert.ok(!dialogs.DIALOG_SAFE_ACTIONS.has('click'));
  });
});
