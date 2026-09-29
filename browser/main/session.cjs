/**
 * The persona's Electron session: the user agent and client hints of its
 * identity (identity.cjs), the persona's proxy, and governance.
 */
const { app } = require('electron');
const governance = require('../governance');
const { watchSession } = require('./observe/install.cjs');
const { configureProxy } = require('../anonymity/proxy');
const { personaIdentity } = require('./identity.cjs');
const { ClientHints } = require('./client-hints.cjs');
const { installPermissions } = require('./permissions.cjs');

/** Configure the persistent browser session, user-agent, cookies, privacy, and what the agent may read back. */
async function configureSession(ses, activeProfile, observer = null) {
  // Telemetry blocking is handled by Chromium flags (applyTelemetryFlags).
  // Domain-level blocking via onBeforeRequest was removed, it interfered
  // with normal page loads and handler stacking on session reuse.
  const identity = personaIdentity(activeProfile, ses.getUserAgent());
  ses.setUserAgent(identity.userAgent, identity.override.acceptLanguage);
  // Service workers never read the session's user agent: they take the app-wide
  // fallback, which is Electron's own ("OyaBrowser/… Electron/…"), in
  // navigator.userAgent and on every request they send. CreepJS reads it off its
  // service worker and flags the page and worker disagreeing.
  app.userAgentFallback = identity.userAgent;
  new ClientHints(identity.hints).install(ses);
  installPermissions(ses);
  await configureProxy(ses, governance.configuration?.proxy || activeProfile?.proxy);
  governance.install(ses);
  if (observer) watchSession(observer, ses);
}

module.exports = { configureSession };
