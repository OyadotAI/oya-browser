/**
 * The persona's Electron session: a user agent and client hints that match
 * the fingerprint and hide Electron, the persona's proxy, and governance.
 */
const governance = require('../governance');
const { configureProxy } = require('../anonymity/proxy');
const { FALLBACK_CHROME_VERSION, GREASE_BRAND_VERSION, GREASE_BRAND_FULL_VERSION } = require('./constants.cjs');

/** The OS part of a Chrome user agent, by the fingerprint's navigator.platform. */
const UA_OS = {
  Win32: 'Windows NT 10.0; Win64; x64',
  MacIntel: 'Macintosh; Intel Mac OS X 10_15_7',
  'Linux x86_64': 'X11; Linux x86_64',
};

/** Sec-CH-UA-Platform, by the fingerprint's navigator.platform. */
const PLATFORM_HINTS = { Win32: 'Windows', 'Linux x86_64': 'Linux', MacIntel: 'macOS' };

/** Sec-CH-UA-Platform for this machine, when the persona names no platform. */
const HOST_HINTS = { darwin: 'macOS', win32: 'Windows' };

/**
 * The session's UA without Electron/oya-browser tokens. The OS portion is
 * rewritten to match the fingerprint profile's platform so the UA and
 * navigator.platform don't contradict each other.
 */
function personaUserAgent(defaultUA, platform, chromeFullVer) {
  if (Object.hasOwn(UA_OS, platform || '')) {
    return `Mozilla/5.0 (${UA_OS[platform]}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${chromeFullVer} Safari/537.36`;
  }
  return defaultUA.replace(/\s*Electron\/[\d.]+/, '').replace(/\s*oya-browser\/[\d.]+/i, '');
}

/** The Sec-CH-UA-Platform value: the persona's platform, else this machine's. */
function platformHintFor(platform) {
  if (Object.hasOwn(PLATFORM_HINTS, platform || '')) return PLATFORM_HINTS[platform];
  return Object.hasOwn(HOST_HINTS, process.platform) ? HOST_HINTS[process.platform] : 'Linux';
}

/** Client-hint header values, by lowercase header name. */
function clientHints(chromeMajor, chromeFullVer, platformHint) {
  return {
    'sec-ch-ua': `"Chromium";v="${chromeMajor}", "Google Chrome";v="${chromeMajor}", "Not:A-Brand";v="${GREASE_BRAND_VERSION}"`,
    'sec-ch-ua-full-version-list': `"Chromium";v="${chromeFullVer}", "Google Chrome";v="${chromeFullVer}", "Not:A-Brand";v="${GREASE_BRAND_FULL_VERSION}"`,
    'sec-ch-ua-platform': `"${platformHint}"`,
    'sec-ch-ua-mobile': '?0',
  };
}

/** Rewrites the client-hint headers of each request to hide Electron. */
function installClientHints(ses, hints) {
  // Calling onBeforeSendHeaders replaces the previous handler (Electron behavior).
  ses.webRequest.onBeforeSendHeaders((details, callback) => {
    const headers = { ...details.requestHeaders };
    for (const key of Object.keys(headers)) {
      const lk = key.toLowerCase();
      if (Object.hasOwn(hints, lk)) headers[key] = hints[lk];
    }
    callback({ requestHeaders: headers });
  });
}

/** Configure the persistent browser session — user-agent, cookies, privacy. */
async function configureSession(ses, activeProfile) {
  // Telemetry blocking is handled by Chromium flags (applyTelemetryFlags).
  // Domain-level blocking via onBeforeRequest was removed — it interfered
  // with normal page loads and handler stacking on session reuse.
  const defaultUA = ses.getUserAgent();
  const platform = activeProfile?.navigator?.platform;
  const chromeFullVer = defaultUA.match(/Chrome\/([\d.]+)/)?.[1] || FALLBACK_CHROME_VERSION;
  ses.setUserAgent(personaUserAgent(defaultUA, platform, chromeFullVer));
  installClientHints(ses, clientHints(chromeFullVer.split('.')[0], chromeFullVer, platformHintFor(platform)));
  await configureProxy(ses, governance.configuration?.proxy || activeProfile?.proxy);
  governance.install(ses);
}

module.exports = { configureSession };
