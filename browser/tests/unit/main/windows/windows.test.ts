/** Multiple windows preserve live pages, isolate shell authority and retain one shared agent target. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { BrowserWindows } from '../../../../src/main/windows/index.ts';
import { ShellIpc } from '../../../../src/main/ipc/handle.ts';
import { mainCtx, FakeWindow } from '../../support/main-ctx.cjs';
import { flush } from '../../support/fakes.cjs';

/** Fake native lifecycle and independent screen positions, without any OS windows. */
class Window extends FakeWindow {
  /** Reflect hidden construction for preparation tests. */
  constructor(options = {}) {
    super();
    this.shown = options.show !== false;
  }
  /** Failed preparation discards its hidden native host. */
  destroy() {
    this.destroyed = true;
    this.emit('closed');
  }
  /** Track the fake window on a multi-monitor desktop. */
  position = { x: 0, y: 0 };
  /** Deliver the native focus event. */
  focus() {
    this.emit('focus');
  }
  /** Close in native event order. */
  close() {
    let prevented = false;
    this.emit('close', {
      preventDefault() {
        prevented = true;
      },
    });
    if (prevented) return;
    this.destroyed = true;
    this.emit('closed');
  }
  /** Return the outer screen rectangle. */
  getBounds() {
    return { ...this.getContentBounds(), ...this.position };
  }
  /** Return a deterministic content area. */
  getContentBounds() {
    return { x: this.position.x, y: this.position.y, width: 1280, height: 800 };
  }
  /** Record screen placement. */
  setPosition(x, y) {
    this.position = { x, y };
  }
  /** Only an open fake window is a drop target. */
  isVisible() {
    return !this.destroyed && this.shown;
  }
}
/** The real window graph over hermetic Electron seams and no live profile. */
function setup(t) {
  const root = mainCtx();
  root.electron.BrowserWindow = Window;
  root.electron.screen = {
    getDisplayNearestPoint: () => ({ workArea: { x: -1600, y: 0, width: 3200, height: 1200 } }),
  };
  root.windows = new BrowserWindows(root);
  const primary = root.windows.current;
  primary.shell.create();
  primary.shell.browsingMode = true;
  t.after(() => {
    root.electron.app.emit('before-quit');
    root.windows.each((scope) => scope.shell.window.close());
  });
  return { root, primary };
}
it('transfers the exact view without loading again, destroying it, or forgetting recording', async (t) => {
  const { root, primary } = setup(t);
  const first = primary.tabs.createTab('https://first.test/');
  const id = primary.tabs.createTab('https://move.test/');
  const tab = primary.tabs.find(id);
  await tab.ready;
  const loads = [...tab.view.webContents.loaded];
  await root.windows.detach(id);
  const target = root.windows.owner(id);
  assert.notEqual(target, primary);
  assert.equal(target.tabs.find(id), tab);
  assert.equal(target.tabs.getActiveView(), tab.view);
  assert.equal(primary.tabs.activeTabId, first);
  assert.deepEqual(tab.view.webContents.loaded, loads);
  assert.equal(tab.view.webContents.isDestroyed(), false);
  assert.deepEqual(root.recorder.forgotten, []);
  assert.equal(root.tabs.list.length, 2);
});
it('routes moved-page title events and keyboard shortcuts to the new owner', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://move.test/');
  const tab = primary.tabs.find(id);
  await tab.ready;
  await root.windows.detach(id);
  const target = root.windows.current;
  tab.view.webContents.emit('page-title-updated', {}, 'Moved title');
  assert.equal(target.shell.window.webContents.sentOn('title-changed').at(-1), 'Moved title');
  tab.view.webContents.emit(
    'before-input-event',
    { preventDefault() {} },
    {
      type: 'keyDown',
      key: 't',
      meta: process.platform === 'darwin',
      control: process.platform !== 'darwin',
    },
  );
  assert.equal(target.tabs.list.length, 2);
  assert.equal(primary.shell.window.isDestroyed(), true);
});
it('allocates unique ids in every window and closes only the requested tab from an agent', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  await root.windows.newWindow();
  const second = root.windows.current;
  const secondId = second.tabs.activeTabId;
  assert.notEqual(id, secondId);
  root.tabs.activateTab(id);
  assert.equal(root.windows.current, primary);
  root.tabs.closeTab(secondId, { keepOne: false });
  assert.equal(primary.tabs.find(id)?.id, id);
  assert.equal(second.tabs.list.length, 0);
  await flush();
});
it('accepts every registered shell main frame, but refuses page contents and shell subframes', async (t) => {
  const { root, primary } = setup(t);
  await root.windows.newWindow();
  const second = root.windows.current;
  const guard = new ShellIpc(root);
  for (const scope of [primary, second]) {
    const sender = scope.shell.window.webContents;
    assert.doesNotThrow(() => guard.requireShell({ sender, senderFrame: sender.mainFrame }));
    assert.throws(() => guard.requireShell({ sender, senderFrame: {} } as any), /Only the Oya workspace/);
  }
  const page = second.tabs.getActiveView().webContents;
  assert.throws(() => guard.requireShell({ sender: page, senderFrame: page.mainFrame }), /Only the Oya workspace/);
});
it('keeps local selection and overlays independent, but broadcasts shared state to every shell', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  await root.windows.newWindow();
  const second = root.windows.current;
  second.overlays.names.add('address');
  assert.equal(primary.overlays.names.size, 0);
  assert.equal(primary.tabs.activeTabId, id);
  root.shell.send('notifications-changed', null);
  assert.deepEqual(primary.shell.window.webContents.sentOn('notifications-changed'), [null]);
  assert.deepEqual(second.shell.window.webContents.sentOn('notifications-changed'), [null]);
  second.overlays.names.clear();
});
it('moves onto another window strip without creating a third window', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  await root.windows.newWindow();
  const second = root.windows.current;
  second.shell.window.setPosition(-1200, 100);
  await root.windows.detach(id, { x: -1100, y: 120 });
  assert.equal(root.windows.owner(id), second);
  assert.equal(primary.shell.window.isDestroyed(), true);
  assert.equal(second.tabs.list.length, 2);
  await flush();
});
it('refuses transfer under agent control or while an overlay owns the page', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  primary.overlays.names.add('address');
  await assert.rejects(() => root.windows.detach(id), /Close the open menu/);
  primary.overlays.names.clear();
  root.control.snapshot = () => ({ interactive: false });
  await assert.rejects(() => root.windows.detach(id), /Take control/);
  assert.equal(root.windows.owner(id), primary);
});
it('shared mutable task fields cannot become shadow copies on a secondary window', async (t) => {
  const { root, primary } = setup(t);
  await root.windows.newWindow();
  const task = new AbortController();
  root.windows.current.chatAbort = task;
  assert.equal(root.chatAbort, task);
  assert.equal(primary.chatAbort, task);
});
it('control shields independently cover both visible pages when an agent takes over', async (t) => {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://one.test/');
  await root.windows.newWindow();
  const second = root.windows.current;
  second.tabs.createTab('https://two.test/');
  const state = { interactive: false, owner: 'agent' };
  root.control.snapshot = () => state;
  root.shield.controlChanged(state);
  for (const scope of [primary, second]) {
    assert.ok(scope.shell.window.getBrowserViews().includes(scope.shield.view));
    assert.equal(scope.shell.window.getBrowserViews().at(-1), scope.shield.view);
  }
});

it('a native window close cannot destroy an agent target, but application quit can', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  root.control.snapshot = () => ({ interactive: false });
  primary.shell.window.close();
  assert.equal(primary.shell.window.isDestroyed(), false);
  assert.equal(root.tabs.find(id)?.id, id);
  root.electron.app.emit('before-quit');
  primary.shell.window.close();
  assert.equal(primary.shell.window.isDestroyed(), true);
});

it('keeps the source page visible until destination chrome has painted', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  let painted;
  const waiting = new Promise((resolve) => {
    painted = resolve;
  });
  const create = root.windows.create.bind(root.windows);
  let target;
  root.windows.create = () => {
    target = create();
    target.shell.painted = () => waiting;
    return target;
  };
  const moving = root.windows.detach(id);
  assert.equal(root.windows.owner(id), primary);
  assert.equal(root.windows.current, primary);
  assert.equal(target.shell.window.isVisible(), false);
  assert.equal(target.tabs.list.length, 0);
  assert.equal(target.shell.stagedTab.id, id);
  painted();
  await moving;
  assert.equal(root.windows.owner(id), target);
  assert.equal(target.shell.window.isVisible(), true);
  assert.equal(target.shell.stagedTab, undefined);
});
it('failed chrome preparation destroys only the hidden destination and retains the live source', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  const tab = primary.tabs.find(id);
  const create = root.windows.create.bind(root.windows);
  let target;
  root.windows.create = () => {
    target = create();
    target.shell.painted = async () => {
      throw new Error('paint failed');
    };
    return target;
  };
  await assert.rejects(root.windows.detach(id), /paint failed/);
  assert.equal(root.windows.owner(id), primary);
  assert.equal(tab.view.webContents.isDestroyed(), false);
  assert.equal(target.shell.window.isDestroyed(), true);
  assert.equal(root.windows.current, primary);
});
