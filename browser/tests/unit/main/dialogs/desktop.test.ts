/** Native production dialog routing preserves ownership, decisions and source isolation. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { DesktopDialogs } from '../../../../src/main/dialogs/index.ts';
/** Test engine replies are not auto-deciding, and debugger access always fails. */
function fixture(initial = true) {
  let human = initial;
  const sheets: any[] = [];
  const notes: unknown[] = [];
  const dialogs = new DesktopDialogs(
    () => human,
    (_page, info, answer) => {
      const sheet = { info, answer, closed: 0 };
      sheets.push(sheet);
      return () => {
        sheet.closed++;
      };
    },
    (...args) => notes.push(args),
  );
  const page = Object.assign(new EventEmitter(), {
    handler: null as any,
    _oyaBeforeUnloadDialogs: true,
    isDestroyed: () => false,
    _setOyaDialogHandler(handler: any) {
      this.handler = handler;
    },
  });
  Object.defineProperty(page, 'debugger', {
    get() {
      assert.fail('Internal CDP forbidden');
    },
  });
  dialogs.watch(page as any);
  return {
    dialogs,
    page,
    sheets,
    notes,
    human(value: boolean) {
      human = value;
    },
    open(type = 'confirm') {
      const replies: unknown[] = [];
      const reply = (...args: unknown[]) => replies.push(args);
      page.handler(
        {
          dialogType: type,
          messageText: 'Continue?',
          defaultPromptText: 'Default',
          frame: { url: 'https://test.invalid' },
        },
        reply,
      );
      return { reply, replies };
    },
  };
}
test('human confirmation is not exposed as an agent decision', async () => {
  const f = fixture();
  const decision = f.open();
  assert.equal(f.dialogs.current(), null);
  assert.equal((await f.dialogs.answer(true)).ok, false);
  assert.deepEqual(decision.replies, []);
  f.sheets[0].answer(false);
  assert.deepEqual(decision.replies, [[false, undefined]]);
  assert.equal(f.sheets[0].closed, 1);
  f.sheets[0].answer(true);
  assert.equal(decision.replies.length, 1);
});
test('agent-to-human transfer preserves a prompt and fences late agent answers', async () => {
  const f = fixture(false);
  const decision = f.open('prompt');
  assert.equal(f.dialogs.current()?.type, 'prompt');
  f.human(true);
  assert.equal((await f.dialogs.answer(true, 'stale')).ok, false);
  assert.deepEqual(decision.replies, []);
  assert.equal(f.sheets[0].info.defaultPromptText, 'Default');
  f.sheets[0].answer(true, 'Human answer');
  assert.deepEqual(decision.replies, [[true, 'Human answer']]);
});
test('human-to-agent transfer closes the sheet without answering the page', async () => {
  const f = fixture();
  const decision = f.open('beforeunload');
  f.human(false);
  f.dialogs.controlChanged();
  f.sheets[0].answer(true);
  assert.deepEqual(decision.replies, []);
  assert.equal(f.sheets[0].closed, 1);
  assert.equal(f.dialogs.current()?.type, 'beforeunload');
  await f.dialogs.answer(false);
  assert.deepEqual(decision.replies, [[false, undefined]]);
  assert.equal(f.sheets[0].closed, 1);
});
test('a stale human UI event rechecks live ownership even without a notification', async () => {
  const f = fixture();
  const decision = f.open();
  f.human(false);
  f.sheets[0].answer(true);
  assert.equal(f.dialogs.current()?.type, 'confirm');
  assert.deepEqual(decision.replies, []);
  await f.dialogs.answer(false);
  assert.deepEqual(decision.replies, [[false, undefined]]);
});
test('engine cancellation closes only its own sheet and never answers it', () => {
  const f = fixture();
  const first = f.open();
  const second = f.open('prompt');
  f.page.emit('-oya-dialog-cancelled', first.reply);
  f.sheets[0].answer(true);
  assert.deepEqual(first.replies, []);
  assert.equal(f.sheets[0].closed, 1);
  assert.equal(f.sheets[1].closed, 0);
  f.sheets[1].answer(false);
  assert.deepEqual(second.replies, [[false, undefined]]);
});
test('alerts are notification-first for either owner and never open a blocking sheet', () => {
  for (const human of [true, false]) {
    const f = fixture(human);
    const alert = f.open('alert');
    assert.deepEqual(alert.replies, [[true, undefined]]);
    assert.equal(f.sheets.length, 0);
    assert.equal(f.notes.length, 1);
  }
});
test('a failed human presenter cancels instead of blocking or accepting', () => {
  const replies: unknown[] = [];
  const dialogs = new DesktopDialogs(
    () => true,
    () => {
      throw Error('UI failed');
    },
  );
  const f = fixture();
  dialogs.watch(f.page as any);
  f.page.handler(
    { dialogType: 'confirm', messageText: 'Continue?', frame: { url: 'about:blank' } },
    (accept: boolean) => replies.push(accept),
  );
  assert.deepEqual(replies, [false]);
});

test('replacement navigation declines an old sheet without accepting unsaved changes', () => {
  const f = fixture();
  const prompt = f.open('prompt');
  const unload = f.open('beforeunload');
  f.page.emit('did-start-navigation', { isMainFrame: true, isSameDocument: true });
  assert.deepEqual(prompt.replies, []);
  f.page.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
  assert.deepEqual(prompt.replies, [[false]]);
  assert.equal(f.sheets[0].closed, 1);
  assert.deepEqual(unload.replies, []);
  assert.equal(f.sheets[1].closed, 0);
});

test('a child-frame navigation does not cancel another frame decision', () => {
  const f = fixture();
  const decision = f.open();
  f.page.emit('did-start-navigation', { isMainFrame: false, isSameDocument: false, frame: {} });
  assert.deepEqual(decision.replies, []);
  assert.equal(f.sheets[0].closed, 0);
});
test('synchronous UI cancellation releases a presentation returned after its callback', () => {
  const f = fixture();
  let closed = 0;
  const dialogs = new DesktopDialogs(
    () => true,
    (_page, _info, reply) => {
      reply(false);
      return () => {
        closed++;
      };
    },
  );
  dialogs.watch(f.page as any);
  f.open();
  assert.equal(closed, 1);
});
