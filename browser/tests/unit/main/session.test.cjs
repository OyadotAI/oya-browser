/**
 * Unit tests for main/session.cjs: the session's user agent and client hints
 * match the persona's platform and never mention Electron. Electron is faked.
 */
const { describe, it, before, after } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { installElectron, freshRequire } = require('../support/fakes.cjs');

const ELECTRON_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) oya-browser/1.0.97 Chrome/134.0.6998.44 Electron/35.1.2 Safari/537.36';

/** A session that records its user agent, header rewriter and proxy. */
function fakeSession(ua = ELECTRON_UA) {
  return {
    ua,
    proxy: null,
    getUserAgent() {
      return this.ua;
    },
    setUserAgent(value, languages) {
      this.ua = value;
      this.languages = languages;
    },
    setPermissionRequestHandler(fn) {
      this.permissionRequest = fn;
    },
    setPermissionCheckHandler(fn) {
      this.permissionCheck = fn;
    },
    async setProxy(value) {
      this.proxy = value;
    },
    webRequest: {
      onBeforeSendHeaders(fn) {
        this.rewrite = fn;
      },
      onHeadersReceived() {},
    },
  };
}

/** The headers the session's rewriter sends for `headers`. */
function rewritten(ses, headers) {
  let out;
  const details = { url: 'https://site.test/', resourceType: 'mainFrame', requestHeaders: headers };
  ses.webRequest.rewrite(details, ({ requestHeaders }) => (out = requestHeaders));
  return out;
}

describe('configureSession', () => {
  let restore;
  let configureSession;
  before(() => {
    restore = installElectron({ app: new EventEmitter() });
    ({ configureSession } = freshRequire('main/session.cjs'));
  });
  after(() => restore());

  it('presents Chrome with no Electron or app token when there is no persona', async () => {
    const ses = fakeSession();
    await configureSession(ses, null);
    assert.ok(!/Electron|oya-browser/i.test(ses.ua));
    assert.match(ses.ua, /Chrome\/134\.0\.0\.0 Safari/);
  });

  it('presents the persona platform in the user agent and in the client hints it sends', async () => {
    const ses = fakeSession();
    await configureSession(ses, { navigator: { platform: 'Win32' } });
    assert.match(ses.ua, /\(Windows NT 10\.0; Win64; x64\).*Chrome\/134\.0\.0\.0/);
    const headers = rewritten(ses, { Accept: '*/*' });
    assert.equal(headers['sec-ch-ua-platform'], '"Windows"');
    assert.match(headers['sec-ch-ua'], /"Google Chrome";v="134"/);
  });

  it("sends the persona's languages with the session, and stops pages being granted the camera unasked", async () => {
    const ses = fakeSession();
    await configureSession(ses, { navigator: { platform: 'Win32', languages: ['fr-FR', 'fr'] } });
    assert.equal(ses.languages, 'fr-FR,fr');
    assert.equal(ses.permissionCheck(null, 'media'), false);
  });

  it('applies the persona proxy, or goes direct without one', async () => {
    const withProxy = fakeSession();
    await configureSession(withProxy, { proxy: { host: 'gate.test', port: 8080 } });
    assert.equal(withProxy.proxy.proxyRules, 'http://gate.test:8080');
    const direct = fakeSession();
    await configureSession(direct, {});
    assert.deepEqual(direct.proxy, { mode: 'direct' });
  });
});
