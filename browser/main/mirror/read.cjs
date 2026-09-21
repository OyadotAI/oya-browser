/**
 * What we read out of the user's launched browser over CDP: its real version
 * (so our headers match theirs) and its cookies, already decrypted by the
 * browser itself. Kept apart from the launch mechanics in capture.cjs.
 */
const { HEADLESS_TOKEN } = require('./constants.cjs');

/** Chromium's sameSite spellings; a cookie without one is unspecified. */
const SAME_SITE = new Set(['Strict', 'Lax', 'None']);

/** The real user agent and Chrome version, with any headless token removed. */
async function readVersion(cdp) {
  const { userAgent, product } = await cdp.send('Browser.getVersion');
  return { userAgent: userAgent.replace(HEADLESS_TOKEN, 'Chrome'), chromeVersion: (product || '').split('/')[1] || '' };
}

/** Every cookie in the profile, decrypted by the browser, in the pool's slim shape. */
async function readCookies(cdp) {
  const { cookies } = await cdp.send('Storage.getCookies');
  return (cookies || []).map(slim);
}

/** One CDP cookie as the server stores it. */
function slim(c) {
  return { name: c.name, value: c.value, domain: c.domain, path: c.path || '/', ...slimFlags(c) };
}

/** The cookie's flags and expiry in the server's spelling; a session cookie carries no expiry. */
function slimFlags(c) {
  return {
    secure: !!c.secure,
    httpOnly: !!c.httpOnly,
    sameSite: SAME_SITE.has(c.sameSite) ? c.sameSite : 'unspecified',
    hostOnly: !c.domain.startsWith('.'),
    ...(c.session || !(c.expires > 0) ? {} : { expirationDate: c.expires }),
  };
}

module.exports = { readVersion, readCookies };
