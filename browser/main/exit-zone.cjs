/**
 * Where a proxied persona's traffic comes out, as a timezone. Sites compare the
 * timezone with where the IP is, and a residential exit moves from session to
 * session, so a persona's fixed zone matched its exit only by luck: Los Angeles
 * over Denver and New York IPs on the browserwars stealth bench. Asked once per
 * session, through that session's proxy, so the answer is the proxy's exit.
 *
 * net.request, not session.fetch: a proxy's auth challenge reaches the app's
 * `login` event only for a tab's requests (anonymity/proxy.js), so a fetch with
 * no tab was refused at the tunnel (ERR_TUNNEL_CONNECTION_FAILED). The request's
 * own `login` event is answered here with the same credentials.
 */
const { net } = require('electron');
const { EXIT_GEO_URL, EXIT_GEO_TIMEOUT_MS } = require('./app/constants.cjs');

/** `zone` when this Chromium knows it as a timezone, otherwise null. */
const knownZone = (zone) => (Intl.supportedValuesOf('timeZone').includes(zone) ? zone : null);

/** The exit's IANA timezone, asked through `ses` and its `proxy`; null when the lookup fails or answers anything else. */
async function exitTimezone(ses, proxy, url = EXIT_GEO_URL) {
  try {
    const zone = knownZone(JSON.parse(await getThrough(ses, proxy, url))?.timezone);
    if (!zone) console.error('[anonymity] exit timezone lookup gave no zone; keeping the persona zone');
    return zone;
  } catch (err) {
    console.error('[anonymity] exit timezone lookup failed, keeping the persona zone:', err?.message || err);
    return null;
  }
}

/** The body of a GET through the session, answering its proxy's challenge, within the deadline. */
function getThrough(ses, proxy, url) {
  return new Promise((resolve, reject) => {
    const req = net.request({ url, session: ses, useSessionCookies: false });
    const done = withDeadline(req, reject);
    answerAuth(req, proxy);
    req.on('response', (res) => readBody(res, done(resolve), done(reject)));
    req.on('error', done(reject));
    req.end();
  });
}

/** Aborts `req` and rejects once the deadline passes; wraps a settle function so it clears the deadline first. */
function withDeadline(req, reject) {
  const timer = setTimeout(() => (req.abort(), reject(new Error('timed out'))), EXIT_GEO_TIMEOUT_MS);
  return (fn) => (value) => (clearTimeout(timer), fn(value));
}

/** Answers the request's proxy challenge with the persona's credentials, as the tabs' is answered. */
function answerAuth(req, proxy) {
  req.on('login', (authInfo, answer) => answer(proxy?.username, proxy?.password || ''));
}

/** Collects a response body as text. */
function readBody(res, resolve, reject) {
  const chunks = [];
  res.on('data', (chunk) => chunks.push(chunk));
  res.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
  res.on('error', reject);
}

module.exports = { exitTimezone };
