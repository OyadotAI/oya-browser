/**
 * The persona's Electron session: the user agent and client hints of its
 * identity (identity.ts), the persona's proxy, and governance.
 */
import type { App, Session } from 'electron';
import { configureProxy, type LoginApp, type ProxyConfig } from '../../anonymity/proxy.ts';
import { watchSession } from '../observe/install.ts';
import type { Observer } from '../observe/observer.ts';
import { personaIdentity, type PersonaProfile } from './identity.ts';
import { ClientHints } from './client-hints.ts';
import { installPermissions } from './permissions.ts';
import type { Governance } from './governance.ts';

/** A persona as the session reads it: its device, and the proxy it goes out through. */
export interface SessionProfile extends PersonaProfile {
  /** The persona's proxy, if it has one (src/anonymity/proxy.ts reads it). */
  proxy?: ProxyConfig | null;
}

/** Electron's app, as far as the session sets it: the fallback user agent, and the proxy's login event. */
type AppFallback = Pick<App, 'userAgentFallback'> & LoginApp;

/** What else a session is set up with, when the app has it. */
export interface SessionExtras {
  /** Collects what pages say and fetch, for the agent to read back. */
  observer?: Observer | null;
  /** The managed egress rules; a governed browser's proxy wins over the persona's. */
  governance?: Governance | null;
}

/**
 * Configure the persistent browser session, user-agent, cookies, privacy, and what
 * the agent may read back. `app` takes the user agent service workers fall back to.
 */
export async function configureSession(
  app: AppFallback,
  ses: Session,
  activeProfile: SessionProfile | null,
  { observer = null, governance = null }: SessionExtras = {},
): Promise<void> {
  presentIdentity(app, ses, activeProfile);
  await routeTraffic(app, ses, activeProfile, governance);
  if (observer) watchSession(observer, ses);
}

/** The persona's proxy (or governance's, which wins), then governance's egress rules. */
async function routeTraffic(
  app: LoginApp,
  ses: Session,
  activeProfile: SessionProfile | null,
  governance: Governance | null,
): Promise<void> {
  await configureProxy(ses, governance?.configuration?.proxy || activeProfile?.proxy, app);
  governance?.install(ses);
}

/** The persona's user agent, client hints and permissions on the session (and the app's fallback). */
function presentIdentity(app: AppFallback, ses: Session, activeProfile: SessionProfile | null): void {
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
}
