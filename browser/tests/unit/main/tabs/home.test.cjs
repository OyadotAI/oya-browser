/**
 * Unit tests for the start page's tab: it shows no web view, ends the moment the
 * view holds anything but its blank page (a navigation starting, even one that
 * fails, or content written into the page), and reopens on the start page.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const {
  isBlank,
  leaveHomeFor,
  staysHome,
  addressOf,
  contentReady,
  wireHome,
} = require('../../../../main/tabs/home.cjs');
const { HOME_URL } = require('../../../../main/tabs/constants.cjs');

describe('the start page tab', () => {
  let tab, ctx, shown;
  beforeEach(() => {
    shown = 0;
    tab = { id: 1, home: true, url: '', view: { webContents: new EventEmitter() } };
    ctx = { tabs: { activeTabId: 1, showInShell: () => shown++ } };
    wireHome(ctx, tab);
  });
  const emit = (name, ...args) => tab.view.webContents.emit(name, ...args);

  it('treats the view’s blank page as no page at all', () => {
    assert.ok(isBlank('') && isBlank('about:blank') && !isBlank('data:text/html,x'));
    assert.ok(staysHome(tab, 'about:blank'));
    assert.equal(leaveHomeFor(tab, 'about:blank'), false);
  });

  it('ends as soon as a navigation starts, so a page that then fails to load is still seen', () => {
    emit('did-start-navigation', { url: 'https://nosuchhost.invalid/', isMainFrame: true });
    assert.equal(tab.home, false);
    assert.equal(shown, 1);
  });

  it('ignores a subframe navigation and the blank page’s own load', () => {
    emit('did-start-navigation', { url: 'https://ads.example/', isMainFrame: false });
    emit('did-start-navigation', { url: 'about:blank', isMainFrame: true });
    emit('did-navigate', {}, 'about:blank');
    emit('dom-ready');
    assert.ok(tab.home);
    assert.equal(shown, 0);
  });

  it('ends when content is written into the blank page with no navigation, as an automation client’s setContent does', () => {
    emit('did-navigate', {}, 'about:blank');
    emit('dom-ready');
    emit('dom-ready');
    assert.equal(tab.home, false);
    assert.equal(shown, 1);
  });

  it('shows the view only if the tab is the one on screen', () => {
    ctx.tabs.activeTabId = 2;
    assert.equal(contentReady(tab), true);
    assert.equal(shown, 0);
  });

  it('reopens on the start page, a used tab on its address, a never-loaded one on about:blank', () => {
    assert.equal(addressOf(tab), HOME_URL);
    assert.equal(addressOf({ url: 'https://a.test/' }), 'https://a.test/');
    assert.equal(addressOf({ url: '' }), 'about:blank');
  });
});
