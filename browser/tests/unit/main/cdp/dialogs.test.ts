/**
 * Unit tests for src/main/cdp/dialogs.ts: alerts are answered at once and reported,
 * a confirm or prompt is held for the driver, and every surface is watched once.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { FakeDebugger, flush } from '../../support/fakes.cjs';
import { Dialogs, DIALOG_HELD, DIALOG_SAFE_ACTIONS, describeDialog } from '../../../../src/main/cdp/dialogs.ts';

describe('dialogs', () => {
  let dialogs: Dialogs;
  let dbg: any;
  beforeEach(() => {
    dialogs = new Dialogs();
    dbg = new FakeDebugger();
    dialogs.watch(dbg);
  });

  it('accepts an alert at once and notes its text for the next result', () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'alert', message: 'Saved' });
    assert.deepEqual(dbg.sent[0], {
      method: 'Page.handleJavaScriptDialog',
      params: { accept: true },
      sessionId: undefined,
    });
    assert.equal(dialogs.takeNotes(), 'Dialog (alert): "Saved", accepted automatically.');
    assert.equal(dialogs.takeNotes(), null, 'notes are taken once');
    assert.equal(dialogs.current(), null);
  });

  it('holds a confirm open and wakes every command waiting on a dialog', async () => {
    const { held } = dialogs.held();
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'Delete?' });
    assert.equal(await held, DIALOG_HELD);
    assert.equal(dialogs.current()?.type, 'confirm');
    assert.equal(dbg.sent.length, 0, 'a decision is not answered automatically');
  });

  it('names the default of a held prompt and says how to unblock it', () => {
    const text = describeDialog({ type: 'prompt', message: 'Name?', defaultPrompt: 'Ann' });
    assert.equal(
      text,
      'A JavaScript prompt dialog is open: "Name?" (default: "Ann"). The page is blocked until you call handle_dialog.',
    );
  });

  it('does not wake a waiter that was released', async () => {
    const { held, release } = dialogs.held();
    release();
    let woke = false;
    held.then(() => (woke = true));
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'x' });
    await flush();
    assert.equal(woke, false);
  });

  it('answers the held prompt with the given text and clears it', async () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'prompt', message: 'Name?' });
    const answer = await dialogs.answer(true, 42);
    assert.deepEqual(dbg.sent.at(-1).params, { accept: true, promptText: '42' });
    assert.deepEqual(answer, { ok: true, data: { type: 'prompt', message: 'Name?', accepted: true } });
    assert.equal(dialogs.current(), null);
  });

  it('dismisses when accept is false and sends no text for a confirm', async () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'Sure?' });
    await dialogs.answer(false, 'ignored');
    assert.deepEqual(dbg.sent.at(-1).params, { accept: false });
  });

  it('reports that no dialog is open when there is none to answer', async () => {
    assert.deepEqual(await dialogs.answer(true), { ok: false, error: 'No dialog is open' });
  });

  it('forgets the held dialog once the page closes it', () => {
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'x' });
    dbg.event('Page.javascriptDialogClosed');
    assert.equal(dialogs.current(), null);
  });

  it('watches a debugger once however often it is attached', () => {
    dialogs.watch(dbg);
    dialogs.watch(null);
    dbg.event('Page.javascriptDialogOpening', { type: 'alert', message: 'once' });
    assert.equal(dbg.sent.length, 1);
  });

  it('keeps screenshots and tab listing answerable while a dialog blocks the page', () => {
    assert.ok(DIALOG_SAFE_ACTIONS.has('screenshot'));
    assert.ok(!DIALOG_SAFE_ACTIONS.has('click'));
  });
});

it('sends only informational alerts to the shell inbox and keeps agent notes', () => {
  const seen = [];
  const dialogs = new Dialogs((message, url) => seen.push({ message, url }));
  const dbg = new FakeDebugger();
  dialogs.watch(dbg);
  for (const type of ['alert', 'beforeunload', 'confirm', 'prompt'])
    dbg.event('Page.javascriptDialogOpening', { type, message: 'Reminder', url: 'https://example.test/' });
  assert.deepEqual(seen, [{ message: 'Reminder', url: 'https://example.test/' }]);
  assert.equal(dbg.sent.length, 2);
  assert.match(dialogs.takeNotes(), /Reminder/);
});

it('closing one legacy surface does not discard a different surface decision', async () => {
  const dialogs = new Dialogs();
  const first = new FakeDebugger();
  const second = new FakeDebugger();
  dialogs.watch(first);
  dialogs.watch(second);
  first.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'First' });
  second.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'Second' });
  second.event('Page.javascriptDialogClosed');
  assert.equal(dialogs.current()?.message, 'First');
  await dialogs.answer(false);
  assert.equal(first.sent.length, 1);
  assert.equal(second.sent.length, 0);
});
