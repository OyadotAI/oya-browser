/**
 * Telemetry removal — disable Google/Chromium phone-home behavior.
 * Must be called BEFORE app.whenReady().
 */

/** Google and Chromium hosts that receive telemetry; subdomains are blocked too. */
const BLOCKED_DOMAINS = [
  'clients1.google.com',
  'clients2.google.com',
  'clients3.google.com',
  'clients4.google.com',
  'clients5.google.com',
  'clients6.google.com',
  'sb-ssl.google.com',
  'safebrowsing.googleapis.com',
  'safebrowsing-cache.google.com',
  'update.googleapis.com',
  'optimizationguide-pa.googleapis.com',
  'content-autofill.googleapis.com',
  'chrome-devtools-frontend.appspot.com',
  'redirector.gvt1.com',
  'accounts.google.com/ListAccounts',
];

/** Chromium features that phone home, switched off together in one --disable-features. */
const DISABLED_FEATURES = [
  'MediaRouter',
  'SafeBrowsing',
  'AutofillServerCommunication',
  'NetworkTimeServiceQuerying',
  'SpareRendererForSitePerProcess',
  'OptimizationHints',
  'Translate',
];

/** Switches that stop background traffic, in the order they are applied. */
const TELEMETRY_SWITCHES = [
  'disable-background-networking',
  'disable-client-side-phishing-detection',
  'disable-default-apps',
  'disable-component-update',
  'disable-domain-reliability',
  'disable-sync',
  'metrics-recording-only',
  'no-pings',
  'disable-breakpad',
  'disable-component-extensions-with-background-pages',
  'disable-hang-monitor',
];

/**
 * Add Chromium command-line flags to disable telemetry.
 * Call before app.whenReady().
 */
function applyTelemetryFlags(app) {
  app.commandLine.appendSwitch('disable-features', DISABLED_FEATURES.join(','));
  for (const name of TELEMETRY_SWITCHES) app.commandLine.appendSwitch(name);
}

/** The webRequest answer for one URL: cancel a telemetry host, let everything else through. */
function blockDecision(rawUrl) {
  try {
    const url = new URL(rawUrl);
    if (url.protocol === 'file:' || url.protocol === 'devtools:') return {};
    const hostname = url.hostname;
    if (BLOCKED_DOMAINS.some((d) => hostname === d || hostname.endsWith('.' + d))) return { cancel: true };
  } catch {}
  return {};
}

/**
 * Block known telemetry domains via webRequest.
 * Safe to call multiple times — clears previous handler first.
 */
function applyDomainBlocking(ses) {
  ses.webRequest.onBeforeRequest(null);
  ses.webRequest.onBeforeRequest((details, callback) => callback(blockDecision(details.url)));
}

module.exports = { applyTelemetryFlags, applyDomainBlocking };
