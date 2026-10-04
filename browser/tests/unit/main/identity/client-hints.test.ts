/**
 * Unit tests for src/main/identity/client-hints.ts: Electron sends no client hints, so the
 * session sends Chrome's, by Chrome's rules.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ClientHints } from '../../../../src/main/identity/client-hints.ts';

const HINTS = {
  'sec-ch-ua': '"Chromium";v="134"',
  'sec-ch-ua-mobile': '?0',
  'sec-ch-ua-platform': '"Windows"',
  'sec-ch-ua-arch': '"x86"',
  'sec-ch-ua-full-version-list': '"Chromium";v="134.0.1.2"',
};

/** A session whose webRequest hooks can be called by hand. */
function fakeSession() {
  const hooks: any = {};
  const webRequest = {
    onBeforeSendHeaders: (fn: any) => (hooks.send = fn),
    onHeadersReceived: (fn: any) => (hooks.received = fn),
  };
  return { webRequest, hooks };
}

describe('ClientHints', () => {
  let ses: any;
  /** The header names a request goes out with. */
  const sent = (details: any) => {
    let out: any;
    ses.hooks.send({ requestHeaders: { Accept: '*/*' }, ...details }, (r: any) => (out = r.requestHeaders));
    return out;
  };
  /** A top-level response from `url` asking for `acceptCh`. */
  const answered = (url: string, acceptCh: string, resourceType = 'mainFrame') => {
    let passed;
    ses.hooks.received({ url, resourceType, responseHeaders: { 'Accept-CH': [acceptCh] } }, (r: any) => (passed = r));
    return passed;
  };

  beforeEach(() => {
    ses = fakeSession();
    new ClientHints(HINTS).install(ses);
  });

  /** The headers Electron hands the hook for a request, in its own order. */
  const ELECTRON = {
    'Upgrade-Insecure-Requests': '1',
    'User-Agent': 'UA',
    'Accept-Language': 'en-US,en;q=0.9',
    Accept: '*/*',
  };

  it('leads a navigation with the three hints, then the language, as Chrome does on the wire', () => {
    const headers = sent({ url: 'https://a.test/', resourceType: 'mainFrame', requestHeaders: ELECTRON });
    assert.deepEqual(Object.keys(headers), [
      'sec-ch-ua',
      'sec-ch-ua-mobile',
      'sec-ch-ua-platform',
      'Accept-Language',
      'Upgrade-Insecure-Requests',
      'User-Agent',
      'Accept',
    ]);
    assert.equal(headers['sec-ch-ua-platform'], '"Windows"');
  });

  it('starts a fetch or subresource with the platform hint and interleaves the rest with the user agent, as Chrome does', () => {
    const { 'Upgrade-Insecure-Requests': _, ...fetchHeaders } = ELECTRON;
    const headers = sent({ url: 'https://a.test/x.js', resourceType: 'script', requestHeaders: fetchHeaders });
    assert.deepEqual(Object.keys(headers), [
      'sec-ch-ua-platform',
      'Accept-Language',
      'sec-ch-ua',
      'User-Agent',
      'sec-ch-ua-mobile',
      'Accept',
    ]);
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
    ses.hooks.send(details, (r: any) => (out = r.requestHeaders));
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
    const from = (page: string) => ({
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
