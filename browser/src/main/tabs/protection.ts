/**
 * Protection for every page surface: the persona's fingerprint and stealth
 * injection, the dialog watcher, the login-state transport, and the isolated
 * world the analyzer runs in. Applied to tabs and to sign-in popups alike.
 */
import type { BrowserWindow, Debugger } from 'electron';
import type { AppServices } from '../app/services.ts';
import { buildInjectionScript } from '../../../anonymity/inject.js';
import { createPersonaApplier, type PersonaProfile } from '../../anonymity/apply.ts';
import { normalizeProxy, type ProxyConfig } from '../../anonymity/proxy.ts';
import { CDP_VERSION, cdpAttach } from '../cdp/cdp.ts';
import { personaIdentity, type UserAgentOverride } from '../identity/identity.ts';
import type { Attempt, TabView } from './types.ts';

/** The services protection uses. */
type Deps = Pick<
  AppServices,
  'persona' | 'config' | 'recorder' | 'world' | 'shield' | 'tabs' | 'dialogs' | 'governance' | 'nativeBrowsing'
>;

/** Hears one CDP event's parameters, and the session it came from. */
type CdpHandler = (params: unknown, sessionId?: string) => void;

/** A CDP connection as the persona applier speaks it: send a command, hear an event. */
export interface CdpPort {
  /** Sends one command, on a child session when `sessionId` is given. */
  send(method: string, params?: object, sessionId?: string): Promise<unknown>;
  /** Listens for one event. */
  on(event: string, fn: CdpHandler): void;
}

/** Says that a protection step failed, and how. */
type Failure = (what: string, err: unknown) => void;

/** A persona as far as protection reads it: the device the applier emulates, its proxy, its canvas. */
interface Profile extends PersonaProfile {
  /** Its own proxy, when it has one. */
  proxy?: ProxyConfig | null;
  /** Its canvas noise. */
  canvas?: object;
}

/** How the applier presents the active persona, for a tab and its workers alike. */
export interface PersonaOptions {
  /** Whether the applier sets the screen size (no: the window owns it). */
  screen: boolean;
  /** The injection's desktop switches. */
  injection: typeof DESKTOP_INJECTION;
  /** The persona, as this exit and this machine should present it. */
  profile: Profile;
  /** The user agent override, metadata included. */
  userAgent: UserAgentOverride;
}

/** A proxy the persona's traffic leaves through, as src/anonymity/proxy.ts normalizes it. */
interface ExitProxy {
  /** The proxy's host; a proxy without one is no proxy. */
  host: string;
  /** The proxy user, often carrying the sticky session id. */
  username?: string;
  /** Its password. */
  password?: string;
}

/** Governance's egress proxy, which wins over the persona's; absent when the browser is not governed. */
type GovernedProxy = ProxyConfig | null | undefined;

/** The app's settings, as far as telling a cloud image from a person's machine goes. */
interface MachineConfig {
  /** Set when this browser runs at a cloud provider. */
  provider?: unknown;
}

/** The message of whatever was thrown. */
const messageOf = (err: unknown): unknown => (err as Error | undefined)?.message || err;

/**
 * A silent failure here would hide which tabs could not be protected. It is
 * said per attempt, not as the tab's verdict: the retry may still protect it,
 * and a tab that stays unprotected is never loaded (tab-events.ts says so).
 */
const tabProtectionFailed: Failure = (what, err) => {
  console.error('[anonymity] ' + what + ' failed, this attempt did not protect the tab:', messageOf(err));
};

/** A step that failed on a tab whose protection held: worth a line, not an alarm. */
const tabStepFailed: Failure = (what, err) => {
  console.error('[anonymity] ' + what + ' failed on this tab:', messageOf(err));
};

/** The same failure, for a popup. */
const popupProtectionFailed: Failure = (what, e) => {
  console.error(`[anonymity] popup ${what} failed, popup is NOT protected:`, messageOf(e));
};

/** Native WebAuthn owns credential requests; the page must never synthesize their cancellation. */
const DESKTOP_INJECTION = { noPasskeyDialog: false, noPermissionPrompt: true, nativeWebRTC: false };
/** How the persona is applied in the desktop app: the window owns the screen, and the injection is the desktop's. */
const DESKTOP_APPLIER = { screen: false, injection: DESKTOP_INJECTION };

/**
 * The persona as this exit should present it. A persona with no proxy leaves by
 * this machine's own connection, so it keeps this machine's timezone: sites
 * compare the timezone with where the IP is, and a persona zone over a home IP
 * reads as "timezone spoofed" (pixelscan and iphey both said so). The zone
 * follows the exit, as a laptop's does when it travels; the device stays the
 * persona's.
 */
function atThisExit(profile: Profile, exitZone: string | null | undefined, governed: GovernedProxy): Profile {
  if (!exitProxy(profile, governed)) return { ...profile, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone };
  return exitZone ? { ...profile, timezone: exitZone } : profile;
}

/** The proxy the persona's traffic leaves through (governance's, `governed`, wins), as src/anonymity/proxy.ts configures it; null when it goes direct. */
export function exitProxy(profile: Profile | null | undefined, governed: GovernedProxy): ExitProxy | null {
  const proxy = normalizeProxy(governed || profile?.proxy);
  return proxy?.host ? { ...proxy, host: proxy.host } : null;
}

/**
 * The persona as a person's own computer should show it: its own screen (the window
 * is really on it, and a size spoofed in JavaScript alone disagrees with matchMedia
 * and the viewport, which iphey reads as an inconsistent fingerprint), and no canvas noise.
 * The GPU here is real, so real rendering is the consistent answer, and any pixel
 * noise can be caught by reading one canvas two ways (pixelscan: "Masking
 * detected"; without it, "consistent"). A cloud or Docker image keeps the noise:
 * identical machines would otherwise all paint one known datacenter hash.
 */
function onThisMachine(profile: Profile, config: MachineConfig | undefined): Profile {
  if (config?.provider || process.env.OYA_DOCKER) return profile;
  return { ...profile, screen: null, canvas: { ...profile.canvas, noiseSeed: null } };
}

/** Subscribes `fn` to one CDP event on a debugger. */
const onDebuggerEvent =
  (dbg: Debugger) =>
  (method: string, fn: (params: unknown) => void): Debugger =>
    dbg.on('message', (_event, event, params) => {
      if (event === method) fn(params);
    });

/**
 * Cross-site iframes attached without pausing them, so a recording can arm them
 * through their own sessions (recording/frame-sessions.ts). A persona attaches
 * them itself, paused until covered; without one nothing else would.
 */
const FRAME_ATTACH = {
  autoAttach: true,
  waitForDebuggerOnStart: false,
  flatten: true,
  filter: [{ type: 'iframe' }, { exclude: true }],
};

/** No persona yet: Chrome's identity in place of Electron's, the stealth injection, and its cross-site iframes. */
async function injectStealthOnly(port: CdpPort, userAgent: UserAgentOverride, fail: Failure): Promise<void> {
  await port.send('Emulation.setUserAgentOverride', userAgent).catch((e) => fail('user agent override', e));
  await port.send('Emulation.setAutomationOverride', { enabled: false }).catch((e) => fail('automation identity', e));
  const source = buildInjectionScript(null, DESKTOP_INJECTION);
  await port.send('Page.addScriptToEvaluateOnNewDocument', { source }).catch((e) => fail('stealth injection', e));
  await port.send('Target.setAutoAttach', FRAME_ATTACH).catch(() => {});
}

/** The persona applier's view of a debugger, sessions included. */
function personaPort(dbg: Debugger): CdpPort {
  return {
    send: (method, params, sessionId) => dbg.sendCommand(method, params, sessionId),
    on: (event, fn) => dbg.on('message', (_e, method, params, sessionId) => method === event && fn(params, sessionId)),
  };
}

/** A port for `x`: a debugger is wrapped, anything that already speaks send/on is one. */
const portOf = (x: Debugger | CdpPort): CdpPort =>
  'sendCommand' in x && typeof x.sendCommand === 'function' ? personaPort(x) : (x as CdpPort);

/** Why a replaced attempt's commands never reach the debugger. */
const REPLACED = 'this protection attempt was replaced by a retry';

/**
 * A debugger port for one setup attempt. A retry detaches and attaches again,
 * and the old attempt, still running, would carry on sending on the new
 * session: each of its steps catches its error and sends the next, the
 * injection included. Fenced, it sends nothing and hears nothing once replaced,
 * so a page is never injected twice.
 */
function fencedPort(dbg: Debugger, attempt: Attempt): CdpPort {
  const port = personaPort(dbg);
  return {
    send: (method, params, sessionId) =>
      attempt.live ? port.send(method, params, sessionId) : Promise.reject(new Error(REPLACED)),
    on: (event, fn) => port.on(event, (params, sessionId) => attempt.live && fn(params, sessionId)),
  };
}

/** A setup attempt's critical failure: it marks the attempt failed and says so, unless a retry already replaced it. */
const criticalIn =
  (attempt: Attempt): Failure =>
  (what, err) => {
    if (!attempt.live) return;
    attempt.failed = true;
    tabProtectionFailed(what, err);
  };

/** A view with no debugger to attach: it is not protected, and says so. */
function notAttached(): false {
  tabProtectionFailed('debugger attach', new Error('view destroyed'));
  return false;
}

/** The analyzer's world could not be rebuilt; the next command loads it. Quiet for a tab that has gone. */
function worldNotRebuilt(view: TabView, err: unknown): void {
  if (view.webContents.isDestroyed?.()) return;
  console.error('[anonymity] isolated world not rebuilt, the next command loads it:', messageOf(err));
}

/** Applies the persona to tabs and popups. */
export class Protection {
  /** The persona, its login state, the recorder, the analyzer's world and the shield. */
  private readonly deps: Deps;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /**
   * The persona on one webContents, applied as the server's CDP driver applies
   * it (src/anonymity/apply.ts): emulation, the injection, and the tab's workers and
   * cross-site iframes. The screen stays the window's own. The user agent is
   * overridden here as well as on the session, because only the override
   * reaches navigator.userAgentData, which the session's string cannot.
   */
  applyPersona(dbg: Debugger | CdpPort, fail: Failure): Promise<unknown> {
    if (this.deps.nativeBrowsing) return Promise.resolve();
    const port = portOf(dbg);
    const persona = this.personaOptions();
    if (!persona) return injectStealthOnly(port, personaIdentity(null).override, fail);
    return createPersonaApplier({ ...port, ...persona, onError: fail }).page();
  }

  /** Only direct, unmanaged browsing may expose native ICE connectivity. */
  private desktopOptions(profile: Profile, active: NonNullable<AppServices['persona']['active']>): PersonaOptions {
    const nativeWebRTC = !this.deps.governance.configuration && !exitProxy(active, null);
    return {
      ...DESKTOP_APPLIER,
      profile,
      userAgent: personaIdentity(active).override,
      injection: { ...DESKTOP_INJECTION, nativeWebRTC },
    };
  }

  /**
   * How the active persona is applied in this browser, the same for its tabs and
   * its workers (workers.ts), so a page and its service worker agree; null when
   * there is no persona yet.
   */
  personaOptions(): PersonaOptions | null {
    if (this.deps.nativeBrowsing) return null;
    const active = this.deps.persona.active;
    if (!active) return null;
    const profile = onThisMachine(
      atThisExit(active, this.deps.persona.exitZone, this.deps.governance.configuration?.proxy),
      this.deps.config?.values,
    );
    return this.desktopOptions(profile, active);
  }

  /**
   * Protects a tab's page, once per view, and answers whether it is protected.
   * Failures are logged loudly, never thrown; a view already being set up
   * answers what its attempt answers.
   */
  async setupTabCDP(view: TabView): Promise<boolean> {
    try {
      this.deps.dialogs.watch(view.webContents);
    } catch (error) {
      tabProtectionFailed('native dialogs', error);
      return false;
    }
    if (this.deps.nativeBrowsing) return !view.webContents.isDestroyed();
    return this.setupPersonaCDP(view);
  }

  /** Persona mode retains the existing fail-closed bounded protection attempt. */
  private async setupPersonaCDP(view: TabView): Promise<boolean> {
    if (!cdpAttach(view)) return notAttached();
    // Main world: only what the page itself must see, as one script in one
    // scope so the toString mask covers the fingerprint patches too. The
    // analyzer is loaded separately into an isolated world by World.ensure().
    // A second caller while an attempt runs (reprotecting every tab on a
    // reconnect) waits for that attempt's answer: an early true would let the
    // first page load before the injection landed.
    if (view.oyaConfigured) return view.oyaSetup as Promise<boolean>;
    view.oyaConfigured = true;
    const attempt: Attempt = (view.oyaAttempt = { live: true, failed: false });
    view.oyaSetup = this.protectTab(view, attempt)
      .catch((e) => criticalIn(attempt)('CDP setup', e))
      .then(() => attempt.live && !attempt.failed);
    return view.oyaSetup;
  }

  /**
   * Lets go of a setup attempt so it can be tried again: the old attempt is
   * fenced off, the recording channel it may have armed is forgotten, and the
   * debugger is detached, which drops every script and override the session
   * held. Listeners stay: the dialog watcher and the recorder's are needed on
   * the next session, and removing them froze a retried tab on its first alert().
   */
  resetTabCDP(view: TabView): void {
    if (view.oyaAttempt) view.oyaAttempt.live = false;
    view.oyaConfigured = false;
    if (this.deps.nativeBrowsing) return;
    this.deps.recorder?.channels?.forget(view);
    try {
      view.webContents.debugger.detach();
    } catch {}
  }

  /** The persona, dialogs, login state and isolated world on one tab's debugger, all through one attempt's port. */
  private async protectTab(view: TabView, attempt: Attempt): Promise<void> {
    const dbg = view.webContents.debugger;
    const port = fencedPort(dbg, attempt);
    await this.applyPersona(port, criticalIn(attempt));
    port.send('Page.enable').catch((e) => tabStepFailed('Page.enable', e));
    const loginState = this.deps.persona.loginState;
    if (loginState) await loginState.attach((method: string, params = {}) => port.send(method, params), port.on);
    this.rebuildWorldOnLoad(view);
  }

  /**
   * A fresh document means a fresh isolated world; rebuild it eagerly so the
   * first command after a navigation does not pay for it. Wired once per view,
   * whatever number of attempts it took. The world is the analyzer's, not the
   * tab's protection, so a failure here says only that, and nothing at all for
   * a tab that was closed mid-load.
   */
  private rebuildWorldOnLoad(view: TabView): void {
    if (view.oyaWorldWired) return;
    view.oyaWorldWired = true;
    view.webContents.on('did-finish-load', () => {
      this.deps.world.ensure(view, { force: true }).catch((e: unknown) => worldNotRebuilt(view, e));
    });
  }

  /**
   * Configure child windows created by allowed popups (OAuth, 2FA, etc.)
   *
   * These must be protected BEFORE the popup's own scripts run. Injecting on
   * did-finish-load meant the document had already executed, and the popup
   * allowlist includes bot-detection vendors, which therefore read a completely
   * unspoofed browser and only saw the overrides afterwards.
   */
  protectPopup(childWindow: BrowserWindow): void {
    this.deps.shield.adoptPopup(childWindow);
    try {
      this.deps.dialogs.watch(childWindow.webContents);
      if (this.deps.nativeBrowsing) return;
      this.protectPopupDebugger(childWindow.webContents.debugger);
    } catch (e) {
      closeUnprotectedPopup(childWindow, e);
    }
  }

  /** The persona, dialogs and login state on a popup's debugger. */
  private protectPopupDebugger(dbg: Debugger): void {
    if (!dbg.isAttached()) dbg.attach(CDP_VERSION);
    this.applyPersona(dbg, popupProtectionFailed);
    dbg.sendCommand('Page.enable').catch(() => {});
    this.syncPopupLogins(dbg);
  }

  /**
   * A sign-in popup is where the session actually gets written, so it needs
   * the same localStorage transport a tab gets. Without this the cookies
   * synced but the token half of a login stayed on this machine.
   */
  private syncPopupLogins(dbg: Debugger): void {
    const loginState = this.deps.persona.loginState;
    if (!loginState) return;
    loginState
      .attach((method: string, params = {}) => dbg.sendCommand(method, params), onDebuggerEvent(dbg))
      .catch((e: Error) => console.error('[oya] popup login state not synced:', e.message));
  }

  /** Ensure the analyzer is loaded in this view's isolated world (fallback, CDP auto-inject is primary). */
  async injectScripts(view?: TabView | null, eager = false): Promise<void> {
    if (eager && this.deps.nativeBrowsing) return;
    if (!view) view = this.deps.tabs.getActiveView();
    if (!view) return;
    try {
      await this.deps.world.ensure(view);
    } catch (e) {
      console.error('[anonymity] isolated world unavailable, analyzer not loaded:', (e as Error).message);
    }
  }
}

/** Refuse to expose a popup whose native decision or persona protections could not be installed. */
function closeUnprotectedPopup(window: BrowserWindow, error: unknown): void {
  window.destroy();
  console.error('[anonymity] popup protection failed; closed unprotected surface:', messageOf(error));
}
