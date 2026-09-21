/**
 * Unit tests for address-bar navigation: the scheme default, cookie pull and
 * protection before the load, and the load skipped when superseded.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { TabManager } = require('../../../../main/tabs/tabs.cjs');
const { normalizeAddress } = require('../../../../main/tabs/navigation.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('address-bar navigation', () => {
  let ctx, tab;
  beforeEach(async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    tab = ctx.tabs.find(ctx.tabs.createTab(''));
    await tab.ready;
  });
  afterEach(() => mock.timers.reset());

  it('adds https to an address typed without a scheme, and trims it', () => {
    assert.equal(normalizeAddress('  example.com '), 'https://example.com');
    assert.equal(normalizeAddress('a.test/x?y=1'), 'https://a.test/x?y=1');
    assert.equal(normalizeAddress('HTTP://a.test'), 'HTTP://a.test');
  });

  it('uses plain http for this machine and bare IPs', () => {
    assert.equal(normalizeAddress('localhost:3000/app'), 'http://localhost:3000/app');
    assert.equal(normalizeAddress('127.0.0.1:8080'), 'http://127.0.0.1:8080');
    assert.equal(normalizeAddress('192.168.1.20'), 'http://192.168.1.20');
  });

  it('opens about:blank as it is', () => {
    assert.equal(normalizeAddress('about:blank'), 'about:blank');
  });

  it('searches for anything that is not an address, other schemes included', () => {
    assert.equal(normalizeAddress('best running shoes'), 'https://www.google.com/search?q=best%20running%20shoes');
    assert.equal(normalizeAddress('oya'), 'https://www.google.com/search?q=oya');
    assert.equal(
      normalizeAddress('file:///etc/passwd'),
      'https://www.google.com/search?q=file%3A%2F%2F%2Fetc%2Fpasswd',
    );
    assert.equal(normalizeAddress('javascript:alert(1)'), 'https://www.google.com/search?q=javascript%3Aalert(1)');
  });

  it('pulls the host cookies, records the move, then loads', async () => {
    const pulled = [];
    ctx.cookies.pullCookiesFor = async (url) => pulled.push(url);
    const recorded = mock.method(ctx.recorder, 'recordNavigation', () => {});
    await ctx.tabs.navigateActive('a.test/x');
    assert.deepEqual(pulled, ['https://a.test/x']);
    assert.deepEqual(recorded.mock.calls[0].arguments, ['https://a.test/x']);
    assert.equal(tab.view.webContents.loaded.at(-1), 'https://a.test/x');
    assert.equal(tab.navigationPending, false);
  });

  it('drops a navigation another one replaced while it waited', async () => {
    let release;
    ctx.cookies.pullCookiesFor = () => new Promise((resolve) => (release = resolve));
    const first = ctx.tabs.navigateActive('first.test');
    tab.navigationRequest++;
    release();
    await first;
    assert.ok(!tab.view.webContents.loaded.includes('https://first.test'));
  });

  it('does not load when control passed to an agent meanwhile', async () => {
    ctx.control.state.interactive = false;
    await ctx.tabs.navigateActive('a.test');
    assert.ok(!tab.view.webContents.loaded.includes('https://a.test'));
    assert.equal(tab.navigationPending, false);
  });

  it('does nothing without an active tab', async () => {
    ctx.tabs.closeTab(tab.id, { keepOne: false });
    await ctx.tabs.navigateActive('a.test');
  });
});
