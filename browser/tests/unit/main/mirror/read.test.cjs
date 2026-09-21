/**
 * Unit tests for mirror/read.cjs: the real browser's cookies come back in the
 * pool's slim shape, session cookies carry no expiry, and a headless user
 * agent is reported as a real one.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { readVersion, readCookies } = require('../../../../main/mirror/read.cjs');

/** A CDP seam that answers each method from a fixed table. */
const fakeCdp = (answers) => ({ send: async (method) => answers[method] });

describe('mirror readVersion', () => {
  it('reports the real Chrome version with the headless token removed', async () => {
    const cdp = fakeCdp({
      'Browser.getVersion': {
        userAgent: 'Mozilla/5.0 HeadlessChrome/128.0.6613.0 Safari/537.36',
        product: 'HeadlessChrome/128.0.6613.0',
      },
    });
    const { userAgent, chromeVersion } = await readVersion(cdp);
    assert.equal(chromeVersion, '128.0.6613.0');
    assert.ok(!/Headless/.test(userAgent));
  });
});

describe('mirror readCookies', () => {
  /** Reads the cookies the fake browser reports and returns them slimmed. */
  const read = (cookies) => readCookies(fakeCdp({ 'Storage.getCookies': { cookies } }));

  it('slims a persistent domain cookie, marking it not host-only with its expiry', async () => {
    const [c] = await read([
      {
        name: 'sid',
        value: 'abc',
        domain: '.example.com',
        path: '/',
        secure: true,
        httpOnly: true,
        sameSite: 'Lax',
        expires: 1893456000,
        session: false,
      },
    ]);
    assert.deepEqual(c, {
      name: 'sid',
      value: 'abc',
      domain: '.example.com',
      path: '/',
      secure: true,
      httpOnly: true,
      sameSite: 'Lax',
      hostOnly: false,
      expirationDate: 1893456000,
    });
  });

  it('keeps a session cookie without an expiry and marks a bare domain host-only', async () => {
    const [c] = await read([
      { name: 'x', value: '1', domain: 'app.example.com', path: '/', session: true, expires: -1 },
    ]);
    assert.equal(c.hostOnly, true);
    assert.equal('expirationDate' in c, false);
    assert.equal(c.sameSite, 'unspecified');
  });
});
