/** Private source sheets deny foreign IPC, revoke on ownership changes, and settle every close or load failure. */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { ShareSheet } from '../../../../src/main/screen-share/window.ts';
import { PICKER } from '../../../../src/main/screen-share/constants.ts';
import { SHARE_PICKER } from '../../../../src/shared/share-picker.ts';

/** Native seams retain the security-sensitive session and IPC callbacks for public-contract checks. */
function fixture(failLoad = false) {
  const state = { current: true, destroyed: false };
  const reads = new Map();
  const ipc = Object.assign(new EventEmitter(), { handle: (name, handler) => reads.set(name, handler) });
  const wc = Object.assign(new EventEmitter(), {
    mainFrame: {},
    ipc,
    setWindowOpenHandler: (handler) => {
      wc.open = handler;
    },
    open: null as any,
  });
  const ses: any = {
    setPermissionRequestHandler: (fn) => {
      ses.permission = fn;
    },
    setPermissionCheckHandler: (fn) => {
      ses.check = fn;
    },
    webRequest: {
      onBeforeRequest: (fn) => {
        ses.request = fn;
      },
    },
  };
  const windows: Window[] = [];
  /** Minimal native window that models synchronous destruction callbacks. */
  class Window extends EventEmitter {
    /** Parent is an actual native window rather than site-provided metadata. */
    static fromWebContents() {
      return {};
    }
    /** Private contents receiving the preload's IPC. */
    webContents = wc;
    /** Observable native creation options. */
    options;
    /** Terminal lifecycle state. */
    destroyed = false;
    /** A sheet must remain hidden until its private preload signals readiness. */
    visible = false;
    /** Store options for permission and isolation assertions. */
    constructor(options) {
      super();
      this.options = options;
      windows.push(this);
    }
    /** A missing asset fails without touching real files. */
    loadFile() {
      return failLoad ? Promise.reject(Error('Missing picker')) : Promise.resolve();
    }
    /** Native lifecycle readback. */
    isDestroyed() {
      return this.destroyed;
    }
    /** Close listeners run synchronously, as in the native implementation. */
    destroy() {
      this.destroyed = true;
      this.emit('closed');
    }
    /** Record first display of the fully populated sheet. */
    show() {
      this.visible = true;
    }
    /** Focus behavior is covered by native integration. */
    focus() {}
  }
  const opened = [];
  const deps = {
    appDir: '/owned/oya',
    electron: {
      shell: {
        openExternal: async (url) => {
          opened.push(url);
        },
      },
      BrowserWindow: Window,
      session: { fromPartition: () => ses },
    },
  };
  const data = { origin: 'https://call.test', sources: [], desktopError: '', canOpenSettings: false };
  const sheet = new ShareSheet(deps as any, {} as any, data, () => {
    if (state.destroyed) throw Error('Object has been destroyed');
    return state.current;
  });
  return { state, wc, ipc, reads, ses, win: windows[0], sheet, data, opened };
}

it('keeps previews and choices inside the exact picker frame and a private unprivileged session', async () => {
  const f = fixture();
  try {
    assert.equal(f.win.options.webPreferences.sandbox, true);
    assert.equal(f.win.options.webPreferences.contextIsolation, true);
    assert.equal(f.win.options.webPreferences.nodeIntegration, false);
    assert.equal(f.ses.check(), false);
    f.ses.permission(null, 'media', (allowed) => assert.equal(allowed, false));
    f.ses.request({ url: 'https://attacker.test/' }, (answer) => assert.deepEqual(answer, { cancel: true }));
    assert.throws(() => f.reads.get(SHARE_PICKER.READ)({ senderFrame: {} }), /no longer available/);
    assert.equal(f.reads.get(SHARE_PICKER.READ)({ senderFrame: f.wc.mainFrame }), f.data);
    f.ipc.emit(SHARE_PICKER.READY, { senderFrame: {} });
    assert.equal(f.win.visible, false);
    f.ipc.emit(SHARE_PICKER.READY, { senderFrame: f.wc.mainFrame });
    f.ipc.emit(SHARE_PICKER.ANSWER, { senderFrame: {} }, 'foreign');
    assert.equal(f.win.destroyed, false);
    f.ipc.emit(SHARE_PICKER.ANSWER, { senderFrame: f.wc.mainFrame }, 'chosen');
    assert.equal(await f.sheet.result, 'chosen');
  } finally {
    if (!f.win.destroyed) f.win.destroy();
  }
});

it('ownership loss cancels the source picker without waiting for its renderer', async () => {
  mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const f = fixture();
  try {
    f.state.current = false;
    mock.timers.tick(PICKER.OWNER_CHECK_MS);
    assert.equal(await f.sheet.result, null);
    assert.equal(f.win.destroyed, true);
  } finally {
    f.win.destroy();
    mock.timers.reset();
  }
});

it('closing, renderer failure and failed loading all cancel without selecting a source', async () => {
  for (const mode of ['close', 'renderer', 'load']) {
    const f = fixture(mode === 'load');
    if (mode === 'close') f.win.destroy();
    if (mode === 'renderer') f.wc.emit('render-process-gone');
    assert.equal(await f.sheet.result, null);
  }
});

it('a picker whose preload never becomes ready closes after the load deadline', async () => {
  mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const f = fixture();
  try {
    mock.timers.tick(PICKER.LOAD_TIMEOUT_MS);
    assert.equal(await f.sheet.result, null);
  } finally {
    f.win.destroy();
    mock.timers.reset();
  }
});

it('only a live main-frame recovery action opens the fixed settings pane and cancels capture', async () => {
  const f = fixture();
  const open = (senderFrame = f.wc.mainFrame) => f.reads.get(SHARE_PICKER.SETTINGS)({ senderFrame });
  try {
    assert.equal(await open(), false);
    f.data.canOpenSettings = true;
    await assert.rejects(open({}), /no longer available/);
    assert.deepEqual(f.opened, []);
    await open();
    assert.deepEqual(f.opened, [PICKER.SETTINGS_URL]);
    assert.equal(await f.sheet.result, null);
    await assert.rejects(open(), /no longer available/);
  } finally {
    f.win.destroy();
  }
});

it('a revoked requester cannot use the settings action', async () => {
  const f = fixture();
  try {
    f.data.canOpenSettings = true;
    f.state.current = false;
    await assert.rejects(f.reads.get(SHARE_PICKER.SETTINGS)({ senderFrame: f.wc.mainFrame }), /no longer available/);
    assert.deepEqual(f.opened, []);
  } finally {
    f.win.destroy();
  }
});

it('native owner teardown settles the picker even if the owner guard throws', async () => {
  const f = fixture();
  f.state.destroyed = true;
  assert.doesNotThrow(() => f.win.destroy());
  assert.equal(await f.sheet.result, null);
});
