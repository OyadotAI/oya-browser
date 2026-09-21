/**
 * Unit tests for the listeners every tab gets: load failures on the strip,
 * view-source lightening, popups versus new tabs, and recording hand-off.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { TabManager } = require('../../../../main/tabs/tabs.cjs');
const { VIEW_SOURCE_LIGHT } = require('../../../../main/tabs/tab-events.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');

describe('tab events', () => {
  let ctx, tab;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
  });
  afterEach(() => mock.timers.reset());

  it('shows a main-frame load failure on the tab', () => {
    tab.navigationPending = true;
    tab.view.webContents.emit('did-fail-load', {}, -105, 'NAME_NOT_RESOLVED', 'https://a.test/', true);
    assert.equal(tab.loadError, 'Page could not load: NAME_NOT_RESOLVED. Try Reload.');
    assert.equal(tab.navigationPending, false);
  });

  it('ignores aborted loads and subframe failures', () => {
    tab.view.webContents.emit('did-fail-load', {}, -3, 'ABORTED', 'https://a.test/', true);
    tab.view.webContents.emit('did-fail-load', {}, -105, 'X', 'https://ad.test/', false);
    assert.equal(tab.loadError, undefined);
  });

  it('shows a crashed renderer, and clears the error when loading starts again', () => {
    tab.view.webContents.emit('render-process-gone', {}, { reason: 'crashed' });
    assert.equal(tab.loadError, 'Page renderer stopped (crashed). Reload to recover.');
    tab.view.webContents.emit('did-start-loading');
    assert.equal(tab.loadError, null);
  });

  it('lightens view-source pages and loads the analyzer on every page', () => {
    const run = mock.method(tab.view.webContents, 'executeJavaScript', async () => {});
    const inject = mock.method(ctx.protection, 'injectScripts', async () => {});
    tab.view.webContents.url = 'view-source:https://a.test/';
    tab.view.webContents.emit('did-finish-load');
    assert.equal(inject.mock.callCount(), 1);
    assert.deepEqual(run.mock.calls[0].arguments, [VIEW_SOURCE_LIGHT, true]);
  });

  it('opens target=_blank links as tabs and lets sign-in popups be windows in the same partition', () => {
    const handler = tab.view.webContents.openHandler;
    assert.deepEqual(handler({ url: 'https://b.test/', features: '' }), { action: 'deny' });
    assert.equal(ctx.tabs.list.length, 2);
    const popup = handler({ url: 'https://accounts.google.com/o/oauth2', features: '' });
    assert.equal(popup.action, 'allow');
    assert.deepEqual(popup.overrideBrowserWindowOptions, {
      width: 500,
      height: 700,
      webPreferences: { partition: 'persist:oya-browser' },
    });
  });

  it('opens a form posted into a new window with its body and referrer, not a bare GET', async () => {
    const postBody = {
      data: [{ type: 'rawData', bytes: Buffer.from('SAMLResponse=abc') }],
      contentType: 'application/x-www-form-urlencoded',
    };
    const referrer = { url: 'https://a.test/portal', policy: 'strict-origin-when-cross-origin' };
    tab.view.webContents.openHandler({ url: 'https://sso.test/saml', features: '', referrer, postBody });
    const opened = ctx.tabs.list.at(-1);
    await opened.ready;
    assert.deepEqual(opened.view.webContents.loadOptions, [
      {
        httpReferrer: referrer,
        postData: postBody.data,
        extraHeaders: 'Content-Type: application/x-www-form-urlencoded',
      },
    ]);
  });

  it("sends a multipart form's boundary with its content type", async () => {
    const postBody = {
      data: [{ type: 'rawData', bytes: Buffer.from('x') }],
      contentType: 'multipart/form-data',
      boundary: 'b1',
    };
    tab.view.webContents.openHandler({ url: 'https://sso.test/', features: '', referrer: { url: '' }, postBody });
    const opened = ctx.tabs.list.at(-1);
    await opened.ready;
    assert.equal(opened.view.webContents.loadOptions[0].extraHeaders, 'Content-Type: multipart/form-data; boundary=b1');
    assert.equal(opened.view.webContents.loadOptions[0].httpReferrer, undefined, 'an empty referrer is not sent');
  });

  it('protects every popup the page creates, and puts it on the tab list', () => {
    const protect = mock.method(ctx.protection, 'protectPopup', () => {});
    const adopt = mock.method(ctx.tabs, 'adoptWindow', () => 2);
    tab.view.webContents.emit('did-create-window', { id: 'popup' });
    assert.deepEqual(protect.mock.calls[0].arguments, [{ id: 'popup' }]);
    assert.deepEqual(adopt.mock.calls[0].arguments, [{ id: 'popup' }]);
  });

  it('keeps a window the page named, because the page keeps using what it opened', () => {
    const before = ctx.tabs.list.length;
    const decision = tab.view.webContents.openHandler({
      url: 'https://vendor.example.com/sso',
      features: '',
      frameName: 'vendorWin',
    });
    assert.equal(decision.action, 'allow');
    assert.equal(ctx.tabs.list.length, before, 'no tab is opened behind its back');
  });

  it('still turns an anonymous target=_blank into a tab', () => {
    const decision = tab.view.webContents.openHandler({
      url: 'https://example.test/page',
      features: '',
      frameName: '_blank',
    });
    assert.equal(decision.action, 'deny');
    assert.equal(ctx.tabs.list.at(-1).url, 'https://example.test/page');
  });

  it('joins a recording once the tab is protected', async () => {
    const join = mock.method(ctx.recorder, 'joinIfRecording', () => {});
    await tab.setup;
    await flush();
    assert.deepEqual(join.mock.calls[0].arguments, [tab.view]);
  });

  it('follows navigations and titles', () => {
    tab.view.webContents.emit('did-navigate', {}, 'https://a.test/2');
    tab.view.webContents.emit('did-navigate-in-page', {}, 'https://a.test/2#x');
    tab.view.webContents.emit('page-title-updated', {}, 'Two');
    assert.equal(tab.url, 'https://a.test/2#x');
    assert.equal(tab.title, 'Two');
  });

  it("drops the previous page's title when the new page has none", () => {
    tab.view.webContents.emit('page-title-updated', {}, 'Cordless drill');
    tab.view.webContents.url = 'about:blank';
    tab.view.webContents.emit('did-finish-load');
    assert.equal(tab.title, 'about:blank');
  });
});
