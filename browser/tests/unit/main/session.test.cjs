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
    setUserAgent(value) {
      this.ua = value;
    },
    async setProxy(value) {
      this.proxy = value;
    },
    webRequest: {
      onBeforeSendHeaders(fn) {
        this.rewrite = fn;
      },
    },
  };
}

/** The headers the session's rewriter sends for `headers`. */
function rewritten(ses, headers) {
  let out;
  ses.webRequest.rewrite({ requestHeaders: headers }, ({ requestHeaders }) => (out = requestHeaders));
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

  it('strips the Electron and app tokens when the persona names no platform', async () => {
    const ses = fakeSession();
    await configureSession(ses, null);
    assert.ok(!/Electron|oya-browser/i.test(ses.ua));
    assert.match(ses.ua, /Chrome\/134\.0\.6998\.44/);
  });

  it('rewrites the OS in the user agent to the persona platform', async () => {
    const ses = fakeSession();
    await configureSession(ses, { navigator: { platform: 'Win32' } });
    assert.equal(
      ses.ua,
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.6998.44 Safari/537.36',
    );
  });

  it('rewrites the client hints to Chrome on the persona platform, whatever their case', async () => {
    const ses = fakeSession();
    await configureSession(ses, { navigator: { platform: 'Linux x86_64' } });
    const headers = rewritten(ses, {
      'Sec-CH-UA': '"Electron"',
      'sec-ch-ua-full-version-list': 'x',
      'sec-ch-ua-platform': '"macOS"',
      'sec-ch-ua-mobile': '?1',
      Accept: '*/*',
    });
    assert.deepEqual(headers, {
      'Sec-CH-UA': '"Chromium";v="134", "Google Chrome";v="134", "Not:A-Brand";v="24"',
      'sec-ch-ua-full-version-list':
        '"Chromium";v="134.0.6998.44", "Google Chrome";v="134.0.6998.44", "Not:A-Brand";v="24.0.0.0"',
      'sec-ch-ua-platform': '"Linux"',
      'sec-ch-ua-mobile': '?0',
      Accept: '*/*',
    });
  });

  it('falls back to a known Chrome version when the user agent names none', async () => {
    const ses = fakeSession('Mozilla/5.0 Electron/35.1.2');
    await configureSession(ses, { navigator: { platform: 'MacIntel' } });
    assert.match(ses.ua, /Chrome\/134\.0\.0\.0/);
    assert.equal(rewritten(ses, { 'sec-ch-ua-platform': '' })['sec-ch-ua-platform'], '"macOS"');
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
