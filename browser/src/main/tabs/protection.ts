/** Native session protection for tabs, frames, popups and every worker, before website execution. */
import type { BrowserWindow, Session } from 'electron';
import { cpus } from 'node:os';
import type { AppServices } from '../app/services.ts';
import { buildInjectionScript, buildWorkerScript } from '../../../anonymity/inject.js';
import { normalizeProxy, type ProxyConfig } from '../../anonymity/proxy.ts';
import { personaIdentity } from '../identity/identity.ts';
import {
  NativeSessionProtection,
  nativePolicyForPersona,
  nativePolicyForHost,
  type ProtectedSession,
  type PreScriptPolicy,
} from '../native-policy/index.ts';
import type { TabView } from './types.ts';
/** Services needed to derive one complete immutable session policy. */
type Deps = Pick<
  AppServices,
  'persona' | 'config' | 'world' | 'shield' | 'tabs' | 'dialogs' | 'governance' | 'nativeBrowsing'
>;
/** A persona's optional outbound proxy. */
interface ProxyProfile {
  /** Explicit persona proxy, superseded by governance. */
  proxy?: ProxyConfig | null;
}
/** Resolve the same normalized outbound proxy used by session networking. */
export function exitProxy(profile: ProxyProfile | null | undefined, governed: ProxyConfig | null | undefined) {
  const proxy = normalizeProxy(governed || profile?.proxy);
  return proxy?.host ? { ...proxy, host: proxy.host } : null;
}
/** Native passkeys and consent remain browser-owned; direct unmanaged ICE stays native. */
const NATIVE_INJECTION = { noPasskeyDialog: false, noPermissionPrompt: true, nativeWebRTC: false };
/** Host-only startup identity before an authenticated persona is available. */
function initialNativeProfile(session: Session) {
  const locale = Intl.DateTimeFormat().resolvedOptions().locale;
  const navigator = {
    platform: personaIdentity(null, session.getUserAgent()).override.platform,
    hardwareConcurrency: cpus().length,
    languages: [locale],
  };
  return { navigator, locale, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone };
}
/** Semantic inputs shared by the document and worker source builders. */
interface NativeSourceInput {
  /** Protected persona, absent before authentication. */
  profile: object | null;
  /** Browser-owned desktop behavior. */
  injection: typeof NATIVE_INJECTION;
  /** Native engine-consistent user agent. */
  userAgent: string;
}
/** Reconnect cache for unchanged semantic script inputs. */
interface ProtectionSources {
  /** Stable serialized identity and desktop policy. */
  key: string;
  /** Exact native sources, including their generated mask keys. */
  scripts: PreScriptPolicy;
}
/** Original source builders run once for an unchanged session identity. */
function buildNativeSources(
  profile: object | null,
  injection: typeof NATIVE_INJECTION,
  userAgent: string,
): PreScriptPolicy {
  return {
    page: buildInjectionScript(profile, injection),
    worker: profile ? buildWorkerScript(profile, { ...injection, userAgent }) : '',
  };
}
/** Install the exact original persona scripts through native engine pre-script hooks. */
export class Protection {
  /** Application-owned policy inputs and surface services. */
  private readonly deps: Deps;
  /** Exact session ownership prevents use of incomplete or changed policies. */
  private readonly sessions = new NativeSessionProtection();
  /** Reuse exact randomized mask sources for identical session policy on reconnect. */
  private readonly sources = new WeakMap<Session, ProtectionSources>();
  /** Share one owner across all application surfaces. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** The persona retains existing desktop screen, canvas and egress-timezone semantics. */
  private profileForSession(session: Session) {
    const active = this.deps.persona.active;
    if (!active) return initialNativeProfile(session);
    const proxy = exitProxy(active, this.deps.governance.configuration?.proxy);
    const timezone = proxy
      ? this.deps.persona.exitZone || active.timezone
      : Intl.DateTimeFormat().resolvedOptions().timeZone;
    const desktop = !this.deps.config?.values.provider && !process.env.OYA_DOCKER;
    return { ...active, timezone, ...(desktop ? { screen: null, canvas: { ...active.canvas, noiseSeed: null } } : {}) };
  }
  /** Must finish before tab constructors, startup workers or private contexts create renderers. */
  configureSession(session: Session): void {
    const profile = this.profileForSession(session);
    const builder = this.deps.persona.active ? nativePolicyForPersona : nativePolicyForHost;
    const policy = builder(profile, session.getUserAgent());
    const nativeWebRTC = !this.deps.governance.configuration && !exitProxy(this.deps.persona.active, null);
    const injection = { ...NATIVE_INJECTION, nativeWebRTC };
    const active = this.deps.persona.active ? profile : null;
    const sources = this.sessionSources(session, { profile: active, injection, userAgent: policy.userAgent });
    this.sessions.configure(session as ProtectedSession, policy, sources);
  }
  /** Build once per semantic identity; randomized masking must not resemble a policy change. */
  private sessionSources(session: Session, input: NativeSourceInput): PreScriptPolicy {
    const { profile, injection, userAgent } = input;
    const key = JSON.stringify({ profile, injection, userAgent });
    const cached = this.sources.get(session);
    if (cached?.key === key) return cached.scripts;
    const scripts = buildNativeSources(profile, injection, userAgent);
    this.sources.set(session, { key, scripts });
    return scripts;
  }

  /** Require a completely protected exact partition before creating or exposing a surface. */
  assertSession(session: Session): void {
    this.sessions.assertConfigured(session as ProtectedSession);
  }
  /** Retain the public method name while all protection is now session-native. */
  async setupTabCDP(view: TabView): Promise<boolean> {
    try {
      this.assertSession(view.webContents.session);
      this.deps.dialogs.watch(view.webContents);
      return !view.webContents.isDestroyed();
    } catch (error) {
      console.error('[anonymity] native tab protection failed:', error);
      return false;
    }
  }
  /** A retry never removes immutable session protection or touches a debugger. */
  resetTabCDP(view: TabView): void {
    if (view.oyaAttempt) view.oyaAttempt.live = false;
    view.oyaConfigured = false;
  }
  /** Popups inherit native protection before their first script; adoption verifies exact ownership. */
  protectPopup(childWindow: BrowserWindow): void {
    try {
      this.assertSession(childWindow.webContents.session);
      this.deps.shield.adoptPopup(childWindow);
      this.deps.dialogs.watch(childWindow.webContents);
    } catch (error) {
      childWindow.destroy();
      console.error('[anonymity] native popup protection failed; closed surface:', error);
    }
  }
  /** The analyzer runs only in its native isolated world. */
  async injectScripts(view?: TabView | null, eager = false): Promise<void> {
    if (eager) return;
    view ||= this.deps.tabs.getActiveView();
    if (!view) return;
    await this.deps.world.ensure(view);
  }
}
