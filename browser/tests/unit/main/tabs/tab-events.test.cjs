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
const { TAB_UNPROTECTED_DESKTOP } = require('../../../../main/tabs/constants.cjs');

describe('tab events', () => {
  let ctx, tab;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
  });
  afterEach(() => mock.timers.reset());

  /** A tab whose setup answers each of `answers` in turn. */
  function tabWith(answers) {
    const reset = mock.method(ctx.protection, 'resetTabCDP', () => {});
    ctx.protection.setupTabCDP = async () => answers.shift();
    return { reset, opened: ctx.tabs.find(ctx.tabs.createTab('https://b.test/')) };
  }

  it('refuses to load a page in a tab whose protection fails twice, and says why on the tab', async () => {
    mock.method(console, 'error', () => {});
    const { reset, opened } = tabWith([false, false]);
    await opened.ready.catch(() => {});
    assert.deepEqual(opened.view.webContents.loaded, ['about:blank']);
    assert.equal(opened.protection, 'failed');
    assert.equal(opened.loadError, TAB_UNPROTECTED_DESKTOP);
    assert.equal(reset.mock.callCount(), 2);
  });

  it('keeps saying the tab is not protected when a load starts in it', async () => {
    mock.method(console, 'error', () => {});
    const { opened } = tabWith([false, false]);
    await opened.ready.catch(() => {});
    opened.view.webContents.emit('did-start-loading');
    assert.equal(opened.loadError, TAB_UNPROTECTED_DESKTOP);
    ctx.tabs.reloadTab(opened);
    assert.equal(opened.loadError, TAB_UNPROTECTED_DESKTOP, 'Reload wiped the reason');
  });

  it('loads the page once when the first attempt fails and the second succeeds', async () => {
    const errors = mock.method(console, 'error', () => {});
    const { reset, opened } = tabWith([false, true]);
    await opened.ready;
    assert.deepEqual(opened.view.webContents.loaded, ['about:blank', 'https://b.test/']);
    assert.equal(opened.protection, 'protected');
    assert.equal(reset.mock.callCount(), 1);
    assert.equal(errors.mock.callCount(), 1);
  });

  it('loads a healthy tab with no retry and no warning', async () => {
    const errors = mock.method(console, 'error', () => {});
    const { reset, opened } = tabWith([true]);
    await opened.ready;
    assert.deepEqual(opened.view.webContents.loaded, ['about:blank', 'https://b.test/']);
    assert.deepEqual([reset.mock.callCount(), errors.mock.callCount()], [0, 0]);
  });

  it('does not join a recording in a tab that was never protected', async () => {
    mock.method(console, 'error', () => {});
    const joined = mock.method(ctx.recorder, 'joinIfRecording', () => {});
    const { opened } = tabWith([false, false]);
    await opened.ready.catch(() => {});
    await flush();
    assert.equal(joined.mock.calls.filter((c) => c.arguments[0] === opened.view).length, 0);
  });

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

  it('never opens a file: address a page asked for in a new tab: the app, unlike the page, would be allowed to read it', () => {
    const handler = tab.view.webContents.openHandler;
    for (const url of ['file:///etc/passwd', ' FILE:///etc/passwd', 'view-source:file:///etc/passwd']) {
      assert.deepEqual(handler({ url, features: '' }), { action: 'deny' }, url);
    }
    assert.equal(ctx.tabs.list.length, 1);
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

  it('joins a sign-in popup to a recording in progress', () => {
    mock.method(ctx.protection, 'protectPopup', () => {});
    const join = mock.method(ctx.recorder, 'joinIfRecording', async () => {});
    const popup = { webContents: tab.view.webContents, on() {} };
    tab.view.webContents.emit('did-create-window', popup);
    assert.equal(join.mock.calls.at(-1).arguments[0], ctx.tabs.list.at(-1).view);
    assert.equal(ctx.tabs.list.at(-1).window, popup);
  });

  it('arms a page again after its renderer died, on the reload that recovers it', async () => {
    await tab.setup;
    const forget = mock.method(ctx.recorder.channels, 'forget', () => {});
    const join = mock.method(ctx.recorder, 'joinIfRecording', async () => {});
    tab.view.webContents.emit('render-process-gone', {}, { reason: 'crashed' });
    assert.deepEqual(forget.mock.calls[0].arguments, [tab.view]);
    tab.view.webContents.emit('did-finish-load');
    assert.deepEqual(join.mock.calls.at(-1).arguments, [tab.view]);
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
