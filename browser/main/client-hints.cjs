/**
 * Client-hint request headers, sent the way Chrome sends them. Electron never
 * sends any (it has no client-hints delegate), so a session whose user agent
 * said Chrome went out with no `sec-ch-ua` at all: something no real Chrome
 * does, and the first thing a sign-in page or an anti-bot vendor checks.
 *
 * Chrome's rules, kept here: hints go to secure origins only; the three
 * low-entropy ones go on every request; a detailed one goes only to an origin
 * whose page asked for it with Accept-CH, and only on that origin's own pages.
 */

/** The hints every secure request carries, in Chrome's order. */
const ALWAYS = ['sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform'];

/** Every hint, in the order Chrome writes them when all are asked for. */
const HINT_ORDER = [
  'sec-ch-ua',
  'sec-ch-ua-mobile',
  'sec-ch-ua-full-version',
  'sec-ch-ua-arch',
  'sec-ch-ua-platform',
  'sec-ch-ua-platform-version',
  'sec-ch-ua-model',
  'sec-ch-ua-bitness',
  'sec-ch-ua-wow64',
  'sec-ch-ua-full-version-list',
];

/** The origin of `url`, or '' when it has none. */
function originOf(url) {
  try {
    return new URL(url).origin;
  } catch {
    return '';
  }
}

/** A header's value whatever its case, as one string; '' when absent. */
function headerValue(headers, name) {
  const key = Object.keys(headers || {}).find((k) => k.toLowerCase() === name);
  return key ? [].concat(headers[key]).join(',') : '';
}

/** One session's client hints: what each origin asked for, and the headers that follow from it. */
class ClientHints {
  /** Origin → the detailed hints its page asked for. ponytail: in memory, so a restart re-learns them on the first response; persist per partition if a site is seen to mind. */
  asked = new Map();

  /** `hints` is the persona's hint values by lowercase header name (identity.cjs). */
  constructor(hints) {
    /** The persona's hint values. */
    this.hints = hints;
  }

  /** Starts sending hints on `ses`, and learning what origins ask for. Replaces the session's previous handlers. */
  install(ses) {
    ses.webRequest.onBeforeSendHeaders((details, callback) => callback({ requestHeaders: this.headersFor(details) }));
    ses.webRequest.onHeadersReceived((details, callback) => {
      this.learn(details);
      callback({});
    });
  }

  /** A top-level page's Accept-CH replaces what its origin asked for. */
  learn(details) {
    if (details.resourceType !== 'mainFrame' || !details.url.startsWith('https:')) return;
    const asked = headerValue(details.responseHeaders, 'accept-ch').toLowerCase().split(',');
    const known = asked.map((name) => name.trim()).filter((name) => Object.hasOwn(this.hints, name));
    this.asked.set(originOf(details.url), new Set(known));
  }

  /** Whether the request goes to the origin of the page making it (or is that page). */
  firstParty(details) {
    if (details.resourceType === 'mainFrame') return true;
    return originOf(details.webContents?.getURL?.() || '') === originOf(details.url);
  }

  /** The hint names this request carries. */
  namesFor(details) {
    const detailed = this.firstParty(details) ? this.asked.get(originOf(details.url)) : null;
    return HINT_ORDER.filter((name) => ALWAYS.includes(name) || detailed?.has(name));
  }

  /** The request's headers with the persona's hints first and any of the engine's own removed. */
  headersFor(details) {
    const entries = Object.entries(details.requestHeaders).filter(([name]) => !/^sec-ch-ua/i.test(name));
    if (!details.url.startsWith('https:')) return Object.fromEntries(entries);
    const hints = this.namesFor(details).map((name) => [name, this.hints[name]]);
    return Object.fromEntries([...hints, ...entries]);
  }
}

module.exports = { ClientHints };
