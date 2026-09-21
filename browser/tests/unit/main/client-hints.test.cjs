/**
 * Unit tests for main/client-hints.cjs: Electron sends no client hints, so the
 * session sends Chrome's, by Chrome's rules.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { ClientHints } = require('../../../main/client-hints.cjs');

const HINTS = {
  'sec-ch-ua': '"Chromium";v="134"',
  'sec-ch-ua-mobile': '?0',
  'sec-ch-ua-platform': '"Windows"',
  'sec-ch-ua-arch': '"x86"',
  'sec-ch-ua-full-version-list': '"Chromium";v="134.0.1.2"',
};

/** A session whose webRequest hooks can be called by hand. */
function fakeSession() {
  const hooks = {};
  const webRequest = {
    onBeforeSendHeaders: (fn) => (hooks.send = fn),
    onHeadersReceived: (fn) => (hooks.received = fn),
  };
  return { webRequest, hooks };
}

describe('ClientHints', () => {
  let ses;
  /** The header names a request goes out with. */
  const sent = (details) => {
    let out;
    ses.hooks.send({ requestHeaders: { Accept: '*/*' }, ...details }, (r) => (out = r.requestHeaders));
    return out;
  };
  /** A top-level response from `url` asking for `acceptCh`. */
  const answered = (url, acceptCh, resourceType = 'mainFrame') => {
    let passed;
    ses.hooks.received({ url, resourceType, responseHeaders: { 'Accept-CH': [acceptCh] } }, (r) => (passed = r));
    return passed;
  };

  beforeEach(() => {
    ses = fakeSession();
    new ClientHints(HINTS).install(ses);
  });

  it("puts Chrome's three hints first on every secure request", () => {
    const headers = sent({ url: 'https://a.test/x.js', resourceType: 'script' });
    assert.deepEqual(Object.keys(headers), ['sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform', 'Accept']);
    assert.equal(headers['sec-ch-ua-platform'], '"Windows"');
  });

  it('sends none over plain http, as Chrome does', () => {
    assert.deepEqual(sent({ url: 'http://a.test/', resourceType: 'mainFrame' }), { Accept: '*/*' });
  });

  it('replaces a hint the engine wrote itself, whatever its case', () => {
    const details = {
      url: 'https://a.test/',
      resourceType: 'mainFrame',
      requestHeaders: { 'Sec-CH-UA': '"Electron"' },
    };
    let out;
    ses.hooks.send(details, (r) => (out = r.requestHeaders));
    assert.deepEqual(out, {
      'sec-ch-ua': HINTS['sec-ch-ua'],
      'sec-ch-ua-mobile': '?0',
      'sec-ch-ua-platform': '"Windows"',
    });
  });

  it('sends a detailed hint only to an origin whose page asked for it, leaving the response alone', () => {
    assert.equal(sent({ url: 'https://a.test/', resourceType: 'mainFrame' })['sec-ch-ua-arch'], undefined);
    assert.deepEqual(answered('https://a.test/', 'Sec-CH-UA-Arch, Sec-CH-UA-Full-Version-List, Viewport-Width'), {});
    const headers = sent({ url: 'https://a.test/next', resourceType: 'mainFrame' });
    assert.equal(headers['sec-ch-ua-arch'], '"x86"');
    assert.equal(headers['sec-ch-ua-full-version-list'], HINTS['sec-ch-ua-full-version-list']);
    assert.equal(sent({ url: 'https://b.test/', resourceType: 'mainFrame' })['sec-ch-ua-arch'], undefined);
  });

  it("keeps detailed hints off another site's requests to that origin", () => {
    answered('https://a.test/', 'Sec-CH-UA-Arch');
    const from = (page) => ({
      url: 'https://a.test/pixel',
      resourceType: 'image',
      webContents: { getURL: () => page },
    });
    assert.equal(sent(from('https://a.test/home'))['sec-ch-ua-arch'], '"x86"');
    assert.equal(sent(from('https://other.test/'))['sec-ch-ua-arch'], undefined);
  });

  it('forgets what an origin asked for when its page stops asking, and ignores subresource responses', () => {
    answered('https://a.test/', 'Sec-CH-UA-Arch');
    answered('https://a.test/app.js', '', 'script');
    assert.equal(sent({ url: 'https://a.test/', resourceType: 'mainFrame' })['sec-ch-ua-arch'], '"x86"');
    answered('https://a.test/', '');
    assert.equal(sent({ url: 'https://a.test/', resourceType: 'mainFrame' })['sec-ch-ua-arch'], undefined);
  });
});
