/** Multiple windows preserve live pages, isolate shell authority and retain one shared agent target. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { BrowserWindows } from '../../../../src/main/windows/index.ts';
import { ShellIpc } from '../../../../src/main/ipc/handle.ts';
import { mainCtx, FakeWindow } from '../../support/main-ctx.cjs';
import { flush } from '../../support/fakes.cjs';
import { Lifecycle } from '../../../../src/main/app/lifecycle.ts';

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
    root.callGuard.commit();
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
it('closes the find bar on a lone tab moved to another window, so the old window still tears down', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  const contents = primary.tabs.find(id).view.webContents;
  const stops = [];
  contents.stopFindInPage = (action) => stops.push(action);
  primary.findBar.open();
  await root.windows.newWindow();
  const second = root.windows.current;
  second.shell.window.setPosition(-1200, 100);
  await root.windows.detach(id, { x: -1100, y: 120 });
  assert.deepEqual(stops, ['clearSelection']);
  assert.equal(contents.listenerCount('found-in-page'), 0);
  let scopes = 0;
  root.windows.each(() => scopes++);
  assert.equal(scopes, 1, 'the closed window left no scope behind');
  await flush();
});
it('refuses transfer under agent control or while an overlay owns the page', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  primary.overlays.names.add('address');
  await assert.rejects(() => root.windows.detach(id), /Close the open menu/);
  primary.overlays.names.clear();
  root.control.snapshot = () => ({ interactive: false });
  await assert.rejects(() => root.windows.detach(id), /Switch to Human Mode/);
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
  root.callGuard.commit();
  primary.shell.window.close();
  assert.equal(primary.shell.window.isDestroyed(), true);
});

it('a window close writes its tabs before closing them', async (t) => {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://one.test/');
  let seen = -1;
  root.sessionTabs.flush = () => (seen = root.tabs.list.length);
  primary.shell.window.close();
  assert.equal(seen, 1, 'the tab was still open at the save');
  assert.equal(root.tabs.list.length, 0);
});

it('a window without an agent tab closes while agents work in their own tabs', async (t) => {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://one.test/');
  root.control.snapshot = () => ({ connected: true, mode: 'agent', interactive: false });
  primary.shell.window.close();
  assert.equal(primary.shell.window.isDestroyed(), true);
});

it('an update restart can close a window the agent holds', async (t) => {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://one.test/');
  root.control.snapshot = () => ({ interactive: false });
  root.electron.autoUpdater.emit('before-quit-for-update');
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

it('detaches a human-owned tab in agent mode while retaining the agent tab and its ownership', async (t) => {
  const { root, primary } = setup(t);
  const agent = primary.tabs.createTab('https://agent.test/');
  const human = primary.tabs.createTab('https://human.test/');
  root.control.snapshot = () => ({ connected: true, mode: 'agent', interactive: false, localClients: 0 });
  root.agentTab.claim(agent);
  await root.windows.detach(human);
  assert.notEqual(root.windows.owner(human), primary);
  assert.equal(root.windows.owner(agent), primary);
  assert.equal(root.agentTab.humanMay(agent), false);
  await assert.rejects(() => root.windows.detach(agent), /Switch to Human Mode/);
});
it('moving a pinned tab to another window unpins it, so it lands in the unpinned block', async (t) => {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://stay.test/');
  const id = primary.tabs.createTab('https://move.test/');
  primary.tabs.find(id).pinned = true;
  await root.windows.detach(id);
  assert.equal(root.windows.owner(id).tabs.find(id).pinned, false);
});

/** Two windows with a tab each, recording how many tabs every session save saw. */
async function twoWindows(t) {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://one.test/');
  await root.windows.newWindow();
  const second = root.windows.current;
  second.tabs.createTab('https://two.test/');
  const saves: number[] = [];
  root.sessionTabs.flush = () => void saves.push(root.tabs.list.length);
  return { root, primary, second, saves };
}

it('a quit saves the session once, with every window open, so the last window to close cannot overwrite it', async (t) => {
  const { root, primary, second, saves } = await twoWindows(t);
  const total = root.tabs.list.length;
  root.callGuard.commit(); // what Lifecycle does on a quit with no call
  second.shell.window.close();
  primary.shell.window.close();
  assert.deepEqual(saves, [total]);
});

it('an update restart, which closes windows before the quit, also saves once with every window open', async (t) => {
  const { root, primary, second, saves } = await twoWindows(t);
  const total = root.tabs.list.length;
  root.electron.autoUpdater.emit('before-quit-for-update');
  primary.shell.window.close();
  second.shell.window.close();
  assert.deepEqual(saves, [total]);
});

it('closing windows one by one without quitting saves at each close', async (t) => {
  const { second, saves } = await twoWindows(t);
  second.shell.window.close();
  assert.equal(saves.length, 1);
});

/** Marks a tab's document as holding a camera, microphone or screen, as a granted call does. */
async function joinCall(root, id) {
  const tab = root.windows.allTabs().find((each) => each.id === id);
  await tab.ready;
  const contents = tab.view.webContents;
  contents.getURL = () => 'https://meet.test/room'; // the fake page never loads its address
  root.mediaPermissions.gate.markLive(contents);
}

it('the red button with a call in this window asks; Cancel keeps the window and the call', async (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://meet.test/room');
  await joinCall(root, id);
  primary.shell.window.close();
  await flush();
  assert.equal(root.electron.dialog.asked.at(-1)[1].message, 'Close this window and end your call?');
  assert.equal(primary.shell.window.isDestroyed(), false);
  assert.equal(primary.tabs.find(id)?.id, id);
});

it('Close Window closes it, and the quit that follows asks nothing more', async (t) => {
  const { root, primary } = setup(t);
  new Lifecycle(root).install();
  await joinCall(root, primary.tabs.createTab('https://meet.test/room'));
  root.electron.dialog.answers.messageBox.push({ response: 1 });
  primary.shell.window.close();
  await flush();
  assert.equal(primary.shell.window.isDestroyed(), true);
  const held = { prevented: false, preventDefault() {} };
  root.electron.app.emit('before-quit', held);
  assert.equal(root.electron.dialog.asked.length, 1, 'one dialog');
});

it('a call in another window does not hold this one', async (t) => {
  const { root, primary } = setup(t);
  primary.tabs.createTab('https://idle.test/');
  const other = root.windows.create();
  await joinCall(root, other.tabs.createTab('https://meet.test/room'));
  primary.shell.window.close();
  assert.equal(primary.shell.window.isDestroyed(), true);
  assert.deepEqual(root.electron.dialog.asked, []);
});

it('the agent hold wins and no call dialog stacks on it', async (t) => {
  const { root, primary } = setup(t);
  await joinCall(root, primary.tabs.createTab('https://meet.test/room'));
  root.control.snapshot = () => ({ interactive: false });
  primary.shell.window.close();
  await flush();
  assert.equal(primary.shell.window.isDestroyed(), false);
  assert.deepEqual(root.electron.dialog.asked, []);
});

it('a cancelled quit keeps the agent close hold', async (t) => {
  const { root, primary } = setup(t);
  new Lifecycle(root).install();
  await joinCall(root, primary.tabs.createTab('https://meet.test/room'));
  root.electron.app.emit('before-quit', { preventDefault() {} });
  await flush();
  assert.equal(root.callGuard.committed, false);
  root.control.snapshot = () => ({ interactive: false });
  primary.shell.window.close();
  assert.equal(primary.shell.window.isDestroyed(), false);
});

/** An incognito session the windows open into, counting how often it retires. */
function withIncognito(root) {
  const session = root.electron.session.fromPartition('oya-incognito-test');
  const state = { retired: 0 };
  root.incognito = { available: () => true, get: async () => session, retire: async () => void state.retired++ };
  return { session, state };
}
/** Opens an incognito window and answers its scope. */
async function openIncognito(root) {
  await root.windows.newIncognitoWindow();
  let found;
  root.windows.each((scope) => scope.tabs.privateSession && (found = scope));
  return found;
}
it('an incognito window never becomes the target of root tab calls, even once focused', async (t) => {
  const { root, primary } = setup(t);
  withIncognito(root);
  const incognito = await openIncognito(root);
  incognito.shell.window.focus();
  assert.equal(root.windows.current, primary);
  const id = root.tabs.createTab('https://agent.test/');
  assert.equal(root.windows.owner(id), primary);
  assert.equal(root.tabs.getActiveView(), primary.tabs.getActiveView());
});
it('agents, session restore and root calls by id never reach an incognito tab', async (t) => {
  const { root } = setup(t);
  withIncognito(root);
  const incognito = await openIncognito(root);
  const id = incognito.tabs.activeTabId;
  assert.equal(
    root.windows.allTabs().some((tab) => tab.id === id),
    false,
  );
  assert.equal(
    root.tabs.list.some((tab) => tab.id === id),
    false,
  );
  assert.equal(root.tabs.find(id), undefined);
  root.tabs.closeTab(id);
  assert.ok(incognito.tabs.find(id));
});
it('a tab opened in an incognito window without a session uses the window session', async (t) => {
  const { root } = setup(t);
  const { session } = withIncognito(root);
  const incognito = await openIncognito(root);
  const id = incognito.tabs.createTab('https://private.test/');
  assert.equal(incognito.tabs.find(id).view.webContents.session, session);
  assert.equal(incognito.tabs.find(incognito.tabs.list[0].id).view.webContents.session, session);
});
it('tabs never move between incognito and normal windows', async (t) => {
  const { root, primary } = setup(t);
  const { session } = withIncognito(root);
  const normal = primary.tabs.createTab('https://normal.test/');
  const incognito = await openIncognito(root);
  const id = incognito.tabs.createTab('https://private.test/');
  assert.throws(() => root.windows.transfer(primary, incognito, primary.tabs.find(normal)), /in or out of incognito/);
  primary.shell.window.setPosition(-1200, 100);
  await root.windows.detach(id, { x: -1100, y: 120 });
  const target = root.windows.owner(id);
  assert.notEqual(target, primary);
  assert.notEqual(target, incognito);
  assert.equal(target.tabs.privateSession, session);
  await flush();
});
it('the person may always use an incognito tab, even while an agent holds the browser', async (t) => {
  const { root } = setup(t);
  withIncognito(root);
  const incognito = await openIncognito(root);
  root.control.snapshot = () => ({ interactive: false, owner: 'agent' });
  assert.equal(root.agentTab.humanMay(incognito.tabs.activeTabId), true);
});
it('closing the last incognito window retires the session and leaves normal windows open', async (t) => {
  const { root, primary } = setup(t);
  const { state } = withIncognito(root);
  const incognito = await openIncognito(root);
  incognito.shell.window.close();
  assert.equal(state.retired, 1);
  assert.equal(primary.shell.window.isDestroyed(), false);
});
it('closing the last normal window closes every incognito window', async (t) => {
  const { root, primary } = setup(t);
  const { state } = withIncognito(root);
  const first = await openIncognito(root);
  await root.windows.newIncognitoWindow();
  primary.shell.window.close();
  assert.equal(first.shell.window.isDestroyed(), true);
  root.windows.each((scope) => assert.fail('a window outlived the last normal one: ' + scope));
  assert.ok(state.retired >= 1, 'the session retired');
});
it('refuses an incognito window before the person has signed in', async (t) => {
  const { root, primary } = setup(t);
  withIncognito(root);
  primary.shell.browsingMode = false;
  await assert.rejects(root.windows.newIncognitoWindow(), /Sign in/);
});
it('a sign-out while incognito is being set up leaves no incognito window and retires the session', async (t) => {
  const { root, primary } = setup(t);
  const { session, state } = withIncognito(root);
  root.incognito.get = async () => {
    primary.shell.browsingMode = false;
    return session;
  };
  await assert.rejects(root.windows.newIncognitoWindow(), /Sign in/);
  root.windows.each((scope) => assert.equal(scope.tabs.privateSession, undefined));
  assert.equal(state.retired, 1, 'no session is left for a later window to reuse');
});
it('closing incognito retires the session even when no incognito window is open yet', async (t) => {
  const { root } = setup(t);
  const { state } = withIncognito(root);
  root.windows.closeIncognito();
  assert.equal(state.retired, 1);
});
