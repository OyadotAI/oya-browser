/**
 * The persona's Electron session: the user agent and client hints of its
 * identity (identity.cjs), the persona's proxy, and governance.
 */
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
  new ClientHints(identity.hints).install(ses);
  installPermissions(ses);
  await configureProxy(ses, governance.configuration?.proxy || activeProfile?.proxy);
  governance.install(ses);
  if (observer) watchSession(observer, ses);
}

module.exports = { configureSession };
