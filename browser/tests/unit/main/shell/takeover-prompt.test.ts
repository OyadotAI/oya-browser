/** Explicit takeover never grants control without an explicit confirmation. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { TakeoverPrompt } from '../../../../src/main/shell/takeover-prompt.ts';
/** Minimal shell and ownership seam. */
function fixture(
  answer: () => Promise<{ /** Native dialog button index. */ response: number }> = async () => ({ response: 0 }),
) {
  const changes: string[] = [];
  let prompts = 0;
  const state = { mode: 'agent', interactive: false, busy: false };
  const deps = {
    control: {
      snapshot: () => state,
      change: async (action: string) => {
        changes.push(action);
      },
    },
    shell: { window: { isDestroyed: () => false } },
    electron: {
      dialog: {
        showMessageBox: async () => {
          prompts++;
          return answer();
        },
      },
    },
  };
  return { prompt: new TakeoverPrompt(deps as any), changes, state, count: () => prompts };
}
it('declining leaves automation in control', async () => {
  const f = fixture();
  await f.prompt.request();
  assert.deepEqual(f.changes, []);
  assert.equal(f.count(), 1);
});
it('accepting acquires control once', async () => {
  const f = fixture(async () => ({ response: 1 }));
  await f.prompt.request();
  assert.deepEqual(f.changes, ['acquire']);
});
it('deduplicates simultaneous requests and rechecks ownership before accepting', async () => {
  let settle!: (answer: { /** Native dialog button index. */ response: number }) => void;
  const f = fixture(
    () =>
      new Promise((resolve) => {
        settle = resolve;
      }),
  );
  const first = f.prompt.request();
  await f.prompt.request();
  assert.equal(f.count(), 1);
  f.state.mode = 'human';
  settle({ response: 1 });
  await first;
  assert.deepEqual(f.changes, []);
});
it('passive movement, clicks and typing never prompt or acquire control', async () => {
  const f = fixture(async () => ({ response: 1 }));
  const contents = new EventEmitter();
  let blocked = false;
  f.prompt.install(contents as any);
  contents.emit('before-mouse-event', {}, { type: 'mouseMove', x: 0, y: 0 });
  contents.emit('before-mouse-event', {}, { type: 'mouseMove', x: 500, y: 0 });
  contents.emit('before-mouse-event', {}, { type: 'mouseDown', x: 500, y: 0 });
  contents.emit(
    'before-input-event',
    {
      preventDefault: () => {
        blocked = true;
      },
    },
    { type: 'keyDown', key: 'a' },
  );
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(f.count(), 0);
  assert.deepEqual(f.changes, []);
  assert.equal(blocked, true);
});
