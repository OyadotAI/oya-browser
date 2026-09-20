/**
 * Unit tests for windows the page opened: a named window stays a window so the
 * page's own script keeps working, and it joins the tab list so an agent can
 * list it, switch to it and close it.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const popups = require('../../../../main/tabs/popup-tabs.cjs');
const { isAuthPopup, opensNamedWindow } = require('../../../../main/auth-popup.cjs');

/** A stand-in for the tab manager: just what the collaborator touches. */
function manager() {
  return {
    list: [],
    nextTabId: 7,
    activeTabId: null,
    sent: 0,
    sendTabList() {
      this.sent += 1;
    },
    titleChanged(tab, title) {
      tab.title = title;
    },
  };
}

/** A stand-in for an Electron window the page opened. */
function fakeWindow(url = 'https://vendor.example.com/sso') {
  const handlers = {};
  return {
    closed: false,
    focused: 0,
    on: (event, fn) => (handlers[event] = fn),
    emit: (event, ...args) => handlers[event]?.(...args),
    focus() {
      this.focused += 1;
    },
    isDestroyed: () => false,
    close() {
      this.closed = true;
    },
    webContents: {
      handlers: {},
      on(event, fn) {
        this.handlers[event] = fn;
      },
      getTitle: () => 'Vendor',
      getURL: () => url,
    },
  };
}

describe('opensNamedWindow', () => {
  it('is true for a target the page can post a form into', () => {
    assert.equal(opensNamedWindow('carelonWin'), true);
  });

  it('is false for the anonymous targets, which name no window', () => {
    for (const target of ['', '_blank', '_self', '_parent', '_top', undefined])
      assert.equal(opensNamedWindow(target), false, `${target} names no window`);
  });

  it('leaves the sign-in popup rule alone', () => {
    assert.equal(isAuthPopup('https://accounts.google.com/o/oauth2', ''), true);
    assert.equal(isAuthPopup('https://vendor.example.com/sso', ''), false);
  });
});

describe('adoptWindow', () => {
  it('lists the window as a tab, active, with its title and url', () => {
    const m = manager();
    const id = popups.adoptWindow(m, fakeWindow());
    assert.equal(id, 7);
    assert.equal(m.activeTabId, 7);
    assert.deepEqual(
      m.list.map((t) => [t.id, t.title, t.url]),
      [[7, 'Vendor', 'https://vendor.example.com/sso']],
    );
  });

  it('gives the tab the window’s webContents, which is what a command drives', () => {
    const m = manager();
    const win = fakeWindow();
    popups.adoptWindow(m, win);
    assert.equal(m.list[0].view.webContents, win.webContents);
  });

  it('adopts a window only once', () => {
    const m = manager();
    const win = fakeWindow();
    popups.adoptWindow(m, win);
    assert.equal(popups.adoptWindow(m, win), null);
    assert.equal(m.list.length, 1);
  });

  it('ignores a window with no page', () => {
    const m = manager();
    assert.equal(popups.adoptWindow(m, {}), null);
    assert.equal(m.list.length, 0);
  });

  it('follows the window’s title', () => {
    const m = manager();
    const win = fakeWindow();
    popups.adoptWindow(m, win);
    win.webContents.handlers['page-title-updated'](null, 'Authorization');
    assert.equal(m.list[0].title, 'Authorization');
  });
});

describe('forgetWindow', () => {
  it('drops the tab when the window closes, and moves active off it', () => {
    const m = manager();
    const win = fakeWindow();
    popups.adoptWindow(m, win);
    win.emit('closed');
    assert.deepEqual(m.list, []);
    assert.equal(m.activeTabId, null);
  });

  it('keeps the other tabs when one popup goes', () => {
    const m = manager();
    m.list.push({ id: 1, title: 'first' });
    m.activeTabId = 1;
    const win = fakeWindow();
    popups.adoptWindow(m, win);
    win.emit('closed');
    assert.deepEqual(
      m.list.map((t) => t.id),
      [1],
    );
    assert.equal(m.activeTabId, 1);
  });
});

describe('raiseWindow and closeWindowTab', () => {
  it('raises the window rather than mounting anything', () => {
    const m = manager();
    const win = fakeWindow();
    popups.adoptWindow(m, win);
    popups.raiseWindow(m, m.list[0]);
    assert.equal(win.focused, 1);
  });

  it('closes the window itself', () => {
    const win = fakeWindow();
    popups.closeWindowTab({ window: win });
    assert.equal(win.closed, true);
  });

  it('says nothing when the window is already gone', () => {
    popups.closeWindowTab({ window: { isDestroyed: () => true, close: () => assert.fail('must not close twice') } });
  });
});
