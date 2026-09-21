/**
 * Unit tests for login-state.js: saved localStorage is restored into each
 * origin until it has been visited, changes are captured, and cookies are
 * translated for Chromium.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { LoginState, cdpCookies } = require('../../login-state.js');
const { flush } = require('./support/fakes.cjs');

/** A page's CDP seam: records commands, keeps listeners, answers by method. */
function fakePage(answers = {}) {
  const sent = [];
  const listeners = {};
  let script = 0;
  const send = async (method, params) => {
    sent.push({ method, params });
    if (method === 'Page.addScriptToEvaluateOnNewDocument') return { identifier: 's' + ++script };
    const answer = answers[method];
    if (answer instanceof Error) throw answer;
    return answer ?? {};
  };
  const on = (event, fn) => (listeners[event] = fn);
  return { send, on, sent, emit: (event, params) => listeners[event](params) };
}

/** The restore script's embedded origins. */
const restored = (page) =>
  JSON.parse(
    page.sent
      .filter((c) => c.method === 'Page.addScriptToEvaluateOnNewDocument')
      .at(-1)
      .params.source.match(/entries = (.*)\[location/)[1],
  );

describe('LoginState', () => {
  it('installs a restore script with the saved origins after enabling the domains', async () => {
    const state = new LoginState({ 'https://a.test': { token: '1' } });
    const page = fakePage();
    await state.attach(page.send, page.on);
    assert.deepEqual(
      page.sent.map((c) => c.method),
      ['Page.enable', 'DOMStorage.enable', 'Page.addScriptToEvaluateOnNewDocument'],
    );
    assert.deepEqual(restored(page), { 'https://a.test': { token: '1' } });
  });

  it('stops restoring an origin once it has been visited, on every page', async () => {
    const state = new LoginState({ 'https://a.test': { t: '1' }, 'https://b.test': { t: '2' } });
    const one = fakePage();
    const two = fakePage();
    await state.attach(one.send, one.on);
    await state.attach(two.send, two.on);
    one.emit('Page.frameNavigated', { frame: { url: 'https://a.test/home' } });
    await flush();
    assert.deepEqual(restored(two), { 'https://b.test': { t: '2' } });
    assert.deepEqual(two.sent.at(-2), {
      method: 'Page.removeScriptToEvaluateOnNewDocument',
      params: { identifier: 's1' },
    });
  });

  it('ignores frame, non-web and unparseable navigations', async () => {
    const state = new LoginState({ 'https://a.test': {} });
    const page = fakePage();
    await state.attach(page.send, page.on);
    const before = page.sent.length;
    page.emit('Page.frameNavigated', { frame: { url: 'https://a.test', parentId: 'p' } });
    page.emit('Page.frameNavigated', { frame: { url: 'about:blank' } });
    page.emit('Page.frameNavigated', { frame: { url: 'not a url' } });
    await flush();
    assert.equal(page.sent.length, before);
  });

  it('captures a changed localStorage and reports it', async () => {
    const changes = [];
    const state = new LoginState({}, (change) => changes.push(change));
    const page = fakePage({ 'DOMStorage.getDOMStorageItems': { entries: [['k', 'v']] } });
    await state.attach(page.send, page.on);
    await page.emit('DOMStorage.domStorageItemUpdated', {
      storageId: { isLocalStorage: true, securityOrigin: 'https://a.test' },
    });
    assert.deepEqual(changes, [{ 'https://a.test': { k: 'v' } }]);
    assert.deepEqual(state.origins, { 'https://a.test': { k: 'v' } });
  });

  it('ignores sessionStorage, non-web origins and storage that is gone', async () => {
    const changes = [];
    const state = new LoginState({}, (change) => changes.push(change));
    const page = fakePage({ 'DOMStorage.getDOMStorageItems': new Error('gone') });
    await state.attach(page.send, page.on);
    await page.emit('DOMStorage.domStorageItemAdded', {
      storageId: { isLocalStorage: false, securityOrigin: 'https://a' },
    });
    await page.emit('DOMStorage.domStorageItemAdded', {
      storageId: { isLocalStorage: true, securityOrigin: 'chrome://x' },
    });
    await page.emit('DOMStorage.domStorageItemsCleared', {
      storageId: { isLocalStorage: true, securityOrigin: 'https://a' },
    });
    assert.deepEqual(changes, []);
  });

  it('stops refreshing a page whose restore script can no longer be installed', async () => {
    const state = new LoginState({});
    const page = fakePage({ 'Page.addScriptToEvaluateOnNewDocument': null });
    page.send = async () => {
      throw new Error('closed');
    };
    await state.attach(async () => ({}), page.on).catch(() => {});
    assert.equal(state.pages.size, 1);
    const [attached] = state.pages;
    attached.send = page.send;
    attached.script = 'x';
    await attached.refresh();
    assert.equal(state.pages.size, 0);
  });
});

describe('cdpCookies', () => {
  it('sets a host-only cookie by URL and a domain cookie by domain', () => {
    const [host, domain] = cdpCookies([
      { name: 'h', value: '1', domain: 'a.test', secure: true, hostOnly: true },
      { name: 'd', value: '2', domain: '.a.test', path: '/p' },
    ]);
    assert.deepEqual(host, { name: 'h', value: '1', path: '/', secure: true, httpOnly: false, url: 'https://a.test/' });
    assert.deepEqual(domain, { name: 'd', value: '2', path: '/p', secure: false, httpOnly: false, domain: '.a.test' });
  });

  it('keeps a positive expiry and translates sameSite from either spelling', () => {
    const [a, b, c] = cdpCookies([
      { name: 'a', value: '', domain: 'x', expirationDate: 10, sameSite: 'no_restriction' },
      { name: 'b', value: '', domain: 'x', expires: -1, sameSite: 'Lax' },
      { name: 'c', value: '', domain: 'x', sameSite: 'unspecified' },
    ]);
    assert.deepEqual([a.expires, a.sameSite], [10, 'None']);
    assert.deepEqual([b.expires, b.sameSite], [undefined, 'Lax']);
    assert.equal('sameSite' in c, false);
  });
});
