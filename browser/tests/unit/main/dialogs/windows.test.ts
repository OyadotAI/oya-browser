/** Private dialog sheets reject foreign frames and release native decisions on every UI failure. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { pathToFileURL } from 'node:url';
import { dialogPresenter } from '../../../../src/main/dialogs/index.ts';
import { NATIVE_DIALOG } from '../../../../src/shared/native-dialog.ts';
/** Browser window/session seam records only native lifecycle and private IPC interactions. */
function fixture(failLoad = false) {
  const answers: unknown[] = [];
  const reads = new Map();
  const ipc = Object.assign(new EventEmitter(), { handle: (name: string, fn: unknown) => reads.set(name, fn) });
  const wc = Object.assign(new EventEmitter(), {
    ipc,
    mainFrame: {},
    setWindowOpenHandler: (handler: unknown) => {
      wc.open = handler;
    },
    open: null as any,
  });
  const session: any = {
    webRequest: {
      onBeforeRequest: (fn: unknown) => {
        session.request = fn;
      },
    },
    setPermissionRequestHandler(fn: unknown) {
      this.permission = fn;
    },
    setPermissionCheckHandler(fn: unknown) {
      this.check = fn;
    },
  };
  const parent = {};
  const windows: Window[] = [];
  /** Native sheet seam with observable creation and teardown. */
  class Window extends EventEmitter {
    /** Resolve the actual native parent without consulting page URLs. */
    static fromWebContents() {
      return parent;
    }
    /** Private IPC and navigation surface. */
    webContents = wc;
    /** Terminal state for idempotent teardown. */
    destroyed = false;
    /** The sheet stays hidden until its trusted preload is ready. */
    visible = false;
    /** The packaged asset selected by the presenter. */
    file = '';
    /** Construction preferences inspected by security assertions. */
    options: any;
    /** Capture one created native window. */
    constructor(options: unknown) {
      super();
      this.options = options;
      windows.push(this);
    }
    /** Resolve or reject loading without filesystem access. */
    loadFile(file: string) {
      this.file = file;
      return failLoad ? Promise.reject(Error('Missing UI')) : Promise.resolve();
    }
    /** Expose native destruction state. */
    isDestroyed() {
      return this.destroyed;
    }
    /** Model Electron closing callbacks synchronously. */
    destroy() {
      this.destroyed = true;
      this.emit('closed');
    }
    /** Record whether complete UI was revealed. */
    show() {
      this.visible = true;
    }
    /** Focus is not simulated in unit tests; native integration verifies it. */
    focus() {}
  }
  const electron = { BrowserWindow: Window, session: { fromPartition: () => session } };
  const present = dialogPresenter({ electron, appDir: '/owned/oya' } as any);
  const close = present(
    {} as any,
    {
      dialogType: 'prompt',
      messageText: '<img>',
      defaultPromptText: 'default',
      frame: { url: 'https://user:secret@site.test/path?token=1' },
    },
    (...args) => answers.push(args),
  );
  return { answers, reads, ipc, wc, session, win: windows[0], parent, close };
}
test('sheet has a private session, restrictive preferences and no navigation or permission grants', () => {
  const f = fixture();
  try {
    assert.equal(f.win.options.parent, f.parent);
    assert.equal(f.win.options.modal, true);
    assert.equal(f.win.options.webPreferences.session, f.session);
    assert.equal(f.win.options.webPreferences.sandbox, true);
    assert.equal(f.win.options.webPreferences.contextIsolation, true);
    assert.equal(f.win.options.webPreferences.nodeIntegration, false);
    assert.equal(f.session.check(), false);
    f.session.permission(null, 'media', (allowed: boolean) => assert.equal(allowed, false));
    f.session.request({ url: 'https://attacker.test' }, (decision: unknown) =>
      assert.deepEqual(decision, { cancel: true }),
    );
    f.session.request({ url: pathToFileURL(f.win.file).href }, (decision: unknown) =>
      assert.deepEqual(decision, { cancel: false }),
    );
    assert.deepEqual(f.wc.open(), { action: 'deny' });
    let prevented = false;
    f.wc.emit('will-navigate', {
      preventDefault() {
        prevented = true;
      },
    });
    assert.equal(prevented, true);
  } finally {
    f.close();
  }
});
test('only the sheet main frame can read, reveal or decide the prompt', () => {
  const f = fixture();
  try {
    assert.throws(() => f.reads.get(NATIVE_DIALOG.READ)({ senderFrame: {} }), /no longer available/);
    const data = f.reads.get(NATIVE_DIALOG.READ)({ senderFrame: f.wc.mainFrame });
    assert.equal(data.origin, 'https://site.test');
    assert.equal(data.message, '<img>');
    f.ipc.emit(NATIVE_DIALOG.READY, { senderFrame: {} });
    assert.equal(f.win.visible, false);
    f.ipc.emit(NATIVE_DIALOG.READY, { senderFrame: f.wc.mainFrame });
    assert.equal(f.win.visible, true);
    f.ipc.emit(NATIVE_DIALOG.ANSWER, { senderFrame: {} }, true, 'foreign');
    f.ipc.emit(NATIVE_DIALOG.ANSWER, { senderFrame: f.wc.mainFrame }, 'true', 'coerced');
    assert.deepEqual(f.answers, []);
    f.ipc.emit(NATIVE_DIALOG.ANSWER, { senderFrame: f.wc.mainFrame }, true, 'chosen');
    assert.deepEqual(f.answers, [[true, 'chosen']]);
    assert.equal(f.win.destroyed, true);
  } finally {
    f.close();
  }
});
test('ownership cancellation closes UI without answering the engine', () => {
  const f = fixture();
  f.close();
  f.close();
  f.ipc.emit(NATIVE_DIALOG.ANSWER, { senderFrame: f.wc.mainFrame }, true, 'late');
  assert.equal(f.win.destroyed, true);
  assert.deepEqual(f.answers, []);
});
test('a sheet closed by the human declines exactly once', () => {
  const f = fixture();
  f.win.destroy();
  assert.deepEqual(f.answers, [[false, '']]);
  f.close();
});
test('renderer failure declines rather than leaving a blocked website', () => {
  const f = fixture();
  f.wc.emit('render-process-gone');
  assert.deepEqual(f.answers, [[false, '']]);
  assert.equal(f.win.destroyed, true);
});
test('missing packaged UI declines and disposes its native window', async () => {
  const f = fixture(true);
  await Promise.resolve();
  assert.deepEqual(f.answers, [[false, '']]);
  assert.equal(f.win.destroyed, true);
});
test('a UI that never finishes loading has a bounded cancellation deadline', (context) => {
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  context.mock.timers.tick(NATIVE_DIALOG.LOAD_TIMEOUT_MS);
  assert.deepEqual(f.answers, [[false, '']]);
  assert.equal(f.win.destroyed, true);
});
