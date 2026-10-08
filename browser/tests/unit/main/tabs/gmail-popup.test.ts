/** Gmail completion waits for a working, protected tab and never rewrites OAuth callbacks. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { GmailPopup, isGmailMailbox, isGoogleSignIn } from '../../../../src/main/tabs/gmail-popup.ts';

/** Resolve microtasks without live web access or OS windows. */
const settle = async () => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};
/** A controlled load promise lets the popup stay alive throughout the handoff. */
function setup(initial = 'https://accounts.google.com/verify') {
  let url = initial,
    closed = false,
    destination = 'https://mail.google.com/mail/u/0/#inbox';
  const ready = Promise.withResolvers();
  const contents = Object.assign(new EventEmitter(), { getURL: () => url });
  const win = {
    webContents: contents,
    isDestroyed: () => closed,
    close: () => {
      closed = true;
    },
  };
  const staged = {
    id: 2,
    ready: ready.promise,
    protection: 'protected',
    view: { webContents: { getURL: () => destination } },
  };
  const tabs = {
    list: [{ id: 1, window: win }],
    activeTabId: 1,
    created: [],
    removed: [],
    find(id) {
      return this.list.find((tab) => tab.id === id);
    },
    createTab(url, activate) {
      this.created.push({ url, activate });
      this.list.push(staged);
      return staged.id;
    },
    closeTab(id) {
      this.removed.push(id);
      this.list = this.list.filter((tab) => tab.id !== id);
    },
    activateTab(id) {
      this.activeTabId = id;
    },
  };
  const control = {
    interactive: true,
    snapshot() {
      return this;
    },
  };
  const shell = { window: { show() {} } };
  new GmailPopup({ tabs, control, shell } as any, win as any, 7, initial);
  return {
    tabs,
    control,
    staged,
    ready,
    closed: () => closed,
    destination: (next) => {
      destination = next;
    },
    load(next) {
      url = next;
      contents.emit('did-navigate', {}, next);
      contents.emit('did-finish-load');
    },
  };
}
it('keeps verification in the popup, then returns the loaded mailbox to the shell exactly once', async () => {
  const t = setup();
  t.load('https://accounts.google.com/challenge');
  assert.equal(t.tabs.created.length, 0);
  const mail = 'https://mail.google.com/mail/u/0/#inbox';
  t.load(mail);
  t.load(mail);
  assert.deepEqual(t.tabs.created, [{ url: mail, activate: false }]);
  assert.equal(t.closed(), false);
  t.ready.resolve();
  await settle();
  assert.equal(t.tabs.activeTabId, 2);
  assert.equal(t.staged.openerId, 7);
  assert.equal(t.closed(), true);
});
it('leaves the verified popup intact when the new tab cannot load', async () => {
  const t = setup();
  t.load('https://mail.google.com/mail/u/0/');
  t.ready.reject(new Error('offline'));
  await settle();
  assert.equal(t.closed(), false);
  assert.deepEqual(t.tabs.removed, [2]);
});
it('does not close the popup if the new tab redirects back to login or loses protection', async () => {
  for (const reason of ['redirect', 'protection', 'control', 'changed']) {
    const t = setup();
    t.load('https://mail.google.com/mail/u/0/');
    if (reason === 'redirect') t.destination('https://accounts.google.com/');
    if (reason === 'protection') t.staged.protection = 'failed';
    if (reason === 'control') t.control.interactive = false;
    if (reason === 'changed') t.load('https://accounts.google.com/');
    t.ready.resolve();
    await settle();
    assert.equal(t.closed(), false, reason);
    assert.deepEqual(t.tabs.removed, [2]);
  }
});
it('leaves third-party OAuth callbacks and ordinary Gmail popups alone', () => {
  const oauth = setup();
  oauth.load('https://app.test/oauth/callback');
  assert.equal(oauth.tabs.created.length, 0);
  const ordinary = setup('https://mail.google.com/mail/u/0/');
  ordinary.load('https://mail.google.com/mail/u/0/');
  assert.equal(ordinary.tabs.created.length, 0);
});
it('does not steal focus from a different tab', async () => {
  const t = setup();
  t.load('https://mail.google.com/mail/u/0/');
  t.tabs.activeTabId = 7;
  t.ready.resolve();
  await settle();
  assert.equal(t.tabs.activeTabId, 7);
});
it('requires exact HTTPS origins and a mailbox path', () => {
  for (const url of [
    'https://mail.google.com.evil.test/mail/',
    'http://mail.google.com/mail/',
    'https://mail.google.com/oauth',
    'https://user:pass@mail.google.com/mail/',
    'garbage',
  ])
    assert.equal(isGmailMailbox(url), false);
  assert.equal(isGoogleSignIn('https://accounts.google.com.evil.test/'), false);
  assert.equal(isGoogleSignIn('https://accounts.google.com/'), true);
});
