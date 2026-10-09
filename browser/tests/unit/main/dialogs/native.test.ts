/** Native dialog coordination preserves decisions across independent page surfaces. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { NativeDialogs, DIALOG_HELD } from '../../../../src/main/dialogs/index.ts';
import type { NativeDialogHandler } from '../../../../src/main/native/dialogs.ts';
/** Engine seam exposing callbacks only; accessing a debugger is always a test failure. */
function page() {
  let handler: NativeDialogHandler | null = null;
  const wc = Object.assign(new EventEmitter(), {
    isDestroyed: () => false,
    _setOyaDialogHandler: (next: NativeDialogHandler | null) => {
      handler = next;
    },
  });
  Object.defineProperty(wc, 'debugger', {
    get() {
      assert.fail('Internal CDP is forbidden');
    },
  });
  return {
    wc,
    open(type: string, message: string) {
      const answers: unknown[] = [];
      const reply = (accept: boolean, text?: string) => {
        answers.push({ accept, text });
      };
      handler!(
        {
          dialogType: type,
          messageText: message,
          defaultPromptText: 'default',
          frame: { url: 'https://fixture.test/' },
        },
        reply,
      );
      return { answers, cancel: () => wc.emit('-oya-dialog-cancelled', reply) };
    },
  };
}
test('simultaneous tab decisions are answered in arrival order', async () => {
  const dialogs = new NativeDialogs();
  const a = page();
  const b = page();
  dialogs.watch(a.wc);
  dialogs.watch(b.wc);
  const first = a.open('confirm', 'First');
  const second = b.open('prompt', 'Second');
  assert.deepEqual(first.answers, []);
  assert.deepEqual(second.answers, []);
  await dialogs.answer(false, 'ignored');
  assert.deepEqual(first.answers, [{ accept: false, text: undefined }]);
  assert.equal(dialogs.current()?.message, 'Second');
  await dialogs.answer(true, 'chosen');
  assert.deepEqual(second.answers, [{ accept: true, text: 'chosen' }]);
  assert.equal(dialogs.current(), null);
});
test('cancelling a background tab never clears another tab decision', async () => {
  const dialogs = new NativeDialogs();
  const a = page();
  const b = page();
  dialogs.watch(a.wc);
  dialogs.watch(b.wc);
  const first = a.open('confirm', 'First');
  const second = b.open('confirm', 'Second');
  second.cancel();
  assert.equal(dialogs.current()?.message, 'First');
  await dialogs.answer(false);
  assert.deepEqual(second.answers, []);
  first.cancel();
  assert.equal(dialogs.current(), null);
});
test('an existing decision wakes a newly waiting command immediately', async () => {
  const dialogs = new NativeDialogs();
  const a = page();
  dialogs.watch(a.wc);
  a.open('confirm', 'Continue?');
  assert.equal(await dialogs.held().held, DIALOG_HELD);
});
test('alerts notify once and never overwrite a pending decision', () => {
  const notes: unknown[] = [];
  const dialogs = new NativeDialogs((text, url) => notes.push({ text, url }));
  const a = page();
  const b = page();
  dialogs.watch(a.wc);
  dialogs.watch(a.wc);
  dialogs.watch(b.wc);
  a.open('confirm', 'Continue?');
  const alert = b.open('alert', 'Saved');
  assert.equal(dialogs.current()?.type, 'confirm');
  assert.equal(alert.answers.length, 1);
  assert.deepEqual(notes, [{ text: 'Saved', url: 'https://fixture.test/' }]);
});
