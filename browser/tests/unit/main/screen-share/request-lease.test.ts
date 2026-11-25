/** Picker leases permit external focus but latch every Oya document, tab, owner and profile revocation. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { ShareRequestLease } from '../../../../src/main/screen-share/request-lease.ts';

/** Native ownership events are injected directly; no browser, user data or network is involved. */
function leaseFixture() {
  const state = { human: true, current: true, destroyed: false, url: 'https://call.test/room' };
  const page = Object.assign(new EventEmitter(), {
    mainFrame: {},
    session: {},
    isDestroyed: () => state.destroyed,
    getURL: () => state.url,
  });
  const window = Object.assign(new EventEmitter(), { webContents: {}, isDestroyed: () => false });
  const tab = { id: 1, view: { webContents: page } };
  const owner = { shell: { window }, tabs: { list: [tab], activeTabId: 1, find: () => tab } };
  const listeners = new Set<() => void>(),
    app = new EventEmitter();
  const deps = {
    electron: { app },
    governance: { configuration: null },
    agentTab: { humanMay: () => state.human },
    persona: { policyEpoch: Symbol(), partitionName: () => 'persist:one' },
    windows: {
      ownerOfContents: (contents) => (contents === page ? owner : undefined),
      fromContents: (contents) => (contents === window.webContents ? owner : undefined),
      tabEvents: {
        subscribe: (listener) => {
          listeners.add(listener);
          return () => listeners.delete(listener);
        },
      },
    },
  };
  const lease = new ShareRequestLease(deps as any, page as any, () => state.current);
  return {
    lease,
    state,
    page,
    window,
    owner,
    tab,
    deps,
    app,
    listeners,
    changed: () => listeners.forEach((listener) => listener()),
  };
}

test('external app and private helper focus preserve the originally selected human requester', () => {
  const f = leaseFixture();
  try {
    f.app.emit('browser-window-focus', {}, { webContents: {} });
    assert.equal(f.lease.current(), true);
    f.state.url = 'https://call.test/room#section';
    f.page.emit('did-start-navigation', { isMainFrame: true, isSameDocument: true });
    assert.equal(f.lease.current(), true);
  } finally {
    f.lease.dispose();
  }
});

test('a brief control claim or tab change stays revoked after ownership returns', () => {
  for (const claim of [true, false]) {
    const f = leaseFixture();
    try {
      if (claim) f.state.human = false;
      else f.owner.tabs.activeTabId = 2;
      f.changed();
      f.state.human = true;
      f.owner.tabs.activeTabId = 1;
      assert.equal(f.lease.current(), false);
    } finally {
      f.lease.dispose();
    }
  }
});

test('another real Oya window cancels even if the original window immediately regains focus', () => {
  const f = leaseFixture(),
    other = { webContents: {} };
  try {
    f.deps.windows.fromContents = () => f.owner;
    f.app.emit('browser-window-focus', {}, other);
    f.app.emit('browser-window-focus', {}, f.window);
    assert.equal(f.lease.current(), false);
  } finally {
    f.lease.dispose();
  }
});

test('document, frame, session, profile, owner and governance changes cannot inherit the picker', () => {
  for (const revoke of [
    (f) => f.page.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false }),
    (f) => {
      f.page.mainFrame = {};
    },
    (f) => {
      f.page.session = {};
    },
    (f) => {
      f.deps.persona.policyEpoch = Symbol();
    },
    (f) => {
      f.deps.persona.partitionName = () => 'persist:two';
    },
    (f) => {
      f.deps.windows.ownerOfContents = () => undefined;
    },
    (f) => {
      f.owner.tabs.find = () => ({ ...f.tab });
    },
    (f) => {
      f.deps.governance.configuration = {};
    },
    (f) => {
      f.state.destroyed = true;
    },
    (f) => {
      f.owner.shell.window = { ...f.window };
    },
  ]) {
    const f = leaseFixture();
    try {
      revoke(f);
      assert.equal(f.lease.current(), false);
    } finally {
      f.lease.dispose();
    }
  }
});

test('settlement releases all request-scoped listeners and permanently revokes the lease', () => {
  const f = leaseFixture();
  f.lease.dispose();
  f.lease.dispose();
  assert.equal(f.lease.current(), false);
  assert.equal(f.listeners.size, 0);
  assert.equal(f.app.listenerCount('browser-window-focus'), 0);
  assert.equal(f.page.listenerCount('did-start-navigation'), 0);
  assert.equal(f.page.listenerCount('destroyed'), 0);
  assert.equal(f.window.listenerCount('closed'), 0);
});
