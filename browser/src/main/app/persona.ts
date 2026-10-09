/**
 * The persona this browser runs as: its fingerprint profile, the Electron
 * session (cookie jar) that belongs to it, and the localStorage transport for
 * its logins. The server is the single source of truth for the profile.
 */
import type { Session } from 'electron';
import type { AppServices } from './services.ts';
import { addressOf } from '../tabs/home.ts';
import { LoginState, type Origins } from '../../page/login-state.ts';
import { ProfileStore } from '../../anonymity/profile-store.ts';
import { configureSession, type SessionProfile, type SessionExtras } from '../identity/session.ts';
import { ExitZone } from '../identity/exit-zone.ts';
import { exitProxy } from '../tabs/protection.ts';
import { NOISE_SEED_DIGITS } from './constants.ts';

/** The services the persona uses. */
type Deps = Pick<
  AppServices,
  | 'nativeBrowsing'
  | 'passkeys'
  | 'mediaPermissions'
  | 'externalApps'
  | 'config'
  | 'electron'
  | 'observer'
  | 'workers'
  | 'socket'
  | 'shell'
  | 'tabs'
  | 'cookies'
  | 'protection'
  | 'governance'
>;

/** A noise seed, absent on a device mirrored from the real machine. */
interface Noise {
  /** The seed, or null when the device renders as it really does. */
  noiseSeed?: number | null;
}

/** The hardware navigator reports. */
interface DeviceNavigator {
  /** navigator.platform. */
  platform?: string;
  /** Logical cores. */
  hardwareConcurrency?: number;
  /** Memory, in GiB as navigator rounds it. */
  deviceMemory?: number;
  /** navigator.languages. */
  languages?: unknown;
}

/** The screen a profile claims. */
interface DeviceScreen {
  /** Width in CSS pixels. */
  width?: number;
  /** Height in CSS pixels. */
  height?: number;
  /** Device pixels per CSS pixel. */
  devicePixelRatio?: number;
}

/** The GPU a profile claims. */
interface DeviceGpu {
  /** The renderer a page reads. */
  renderer?: string;
  /** The real GPU name. */
  unmaskedRenderer?: string;
}

/** A persona's font list. */
interface DeviceFonts {
  /** The fonts it has. */
  available?: string[];
}

/** A fingerprint profile as the server sends it (anonymity/fingerprint.js builds the page side from it). */
export interface FingerprintProfile extends SessionProfile {
  /** The persona's id; it names the session partition. */
  id: string;
  /** The hardware navigator reports. */
  navigator: DeviceNavigator;
  /** The screen it claims. */
  screen: DeviceScreen;
  /** The GPU it claims. */
  webgl: DeviceGpu;
  /** Its IANA timezone. */
  timezone?: string;
  /** Its locale. */
  locale?: string;
  /** Its font list; a mirrored device has none. */
  fonts?: DeviceFonts;
  /** Its canvas noise. */
  canvas?: Noise;
  /** Its audio noise. */
  audio?: Noise;
  /** Everything else the profile carries. */
  [key: string]: unknown;
}

/** The fingerprint debug bar's view of a profile. */
export type FingerprintSummary = Record<string, unknown>;

/** What auth_ok carries for the login-state transport. */
export interface LoginStateMessage {
  /** Each origin's saved localStorage. */
  origins?: Origins;
  /** The persona it belongs to. */
  fingerprint?: Pick<FingerprintProfile, 'id'>;
}

/** What the fingerprint debug bar shows for a profile. */
export function fingerprintSummary(profile: FingerprintProfile): FingerprintSummary {
  return { id: profile.id, ...deviceSummary(profile), ...localeSummary(profile) };
}

/** The hardware a profile claims. */
function deviceSummary(profile: FingerprintProfile): FingerprintSummary {
  return {
    platform: profile.navigator.platform,
    hardwareConcurrency: profile.navigator.hardwareConcurrency,
    deviceMemory: profile.navigator.deviceMemory,
    screen: `${profile.screen.width}x${profile.screen.height}`,
    dpr: profile.screen.devicePixelRatio,
    gpu: profile.webgl.unmaskedRenderer,
  };
}

/** A noise seed as shown; a device mirrored from the real machine has none, and renders as it really does. */
const noiseShown = (seed: unknown): string => (typeof seed === 'number' ? seed.toFixed(NOISE_SEED_DIGITS) : 'real');

/**
 * Where a profile claims to be, and its noise seeds. A mirrored device carries no
 * font list and no noise: reading them unguarded threw inside auth_ok, which closed
 * the connection for good the moment an import succeeded.
 */
function localeSummary(profile: FingerprintProfile): FingerprintSummary {
  return {
    timezone: profile.timezone,
    locale: profile.locale,
    fonts: profile.fonts?.available?.length || 0,
    canvasNoise: noiseShown(profile.canvas?.noiseSeed),
    audioNoise: noiseShown(profile.audio?.noiseSeed),
  };
}

/** The active persona and everything bound to it. */
export class Persona {
  /** The fingerprint profile in use, or null before the server sends one. */
  active: FingerprintProfile | null = null;
  /** Saved profiles on disk. */
  store: ProfileStore<FingerprintProfile> | null = null;
  /** localStorage sync for this persona's logins, once the server has authenticated us. */
  loginState: LoginState | null = null;
  /** Where a proxied persona's traffic comes out, as a timezone; null when direct or unknown. */
  exitZone: string | null = null;
  /** The main-process services. */
  private readonly deps: Deps;
  /** Asks a proxied session where it comes out. */
  private readonly zone: ExitZone;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.zone = new ExitZone({ net: deps.electron.net });
  }

  /** Loads the saved profile store and the profile last in use. */
  loadActive(userDataPath: string): void {
    this.store = new ProfileStore<FingerprintProfile>(userDataPath);
    const activeId = this.deps.config.values.activeProfileId || this.store.getActiveId();
    if (activeId) this.active = this.store.get(activeId);
  }

  /** Each persona has its own partition, so its cookies never mix with another's. */
  partitionName(): string {
    if (this.active) return `persist:oya-${this.active.id}`;
    return 'persist:oya-browser';
  }

  /** The Electron session for the active persona. */
  session(): Session {
    return this.deps.electron.session.fromPartition(this.partitionName());
  }

  /**
   * Writes this jar's cookies and storage to disk now. Chromium otherwise writes
   * on a timer, and a login made just before quit or an update restart was lost.
   */
  async flushJar(): Promise<void> {
    const session = this.session();
    await Promise.allSettled([session.cookies.flushStore(), session.flushStorageData()]);
  }

  /** Keep protocol prompting injected through the persona session boundary. */
  private sessionExtras(): SessionExtras {
    const { observer, governance, mediaPermissions: media } = this.deps;
    return {
      nativeBrowsing: this.deps.nativeBrowsing,
      observer,
      governance,
      media,
      externalApp: (url, contents) => void this.deps.externalApps.request(url, contents),
    };
  }

  /** Configure the persistent browser session, user-agent, cookies, privacy. */
  async setupBrowserSession(): Promise<void> {
    const { governance } = this.deps;
    this.deps.passkeys?.install(this.session());
    await configureSession(this.deps.electron.app, this.session(), this.active, this.sessionExtras());
    // Only a proxied persona asks: without one the zone is this machine's own, known already.
    const proxy = exitProxy(this.active, governance.configuration?.proxy);
    this.exitZone = proxy ? await this.zone.timezone(this.session(), proxy) : null;
    await this.deps.workers?.cover();
  }

  /** The fingerprint bar's view of the active profile, or null. */
  summary(): FingerprintSummary | null {
    if (!this.active) return null;
    return fingerprintSummary(this.active);
  }

  /** A new LoginState for a new persona; the same persona keeps the one it has. */
  ensureLoginState(msg: LoginStateMessage): void {
    if (this.loginState && this.active?.id === msg.fingerprint?.id) return;
    const changed = (origins: Origins): void => {
      const socket = this.deps.socket;
      if (socket.ready && socket.isOpen()) socket.send({ type: 'storage_changed', origins });
    };
    this.loginState = new LoginState(msg.origins || {}, changed);
  }

  /**
   * Apply a fingerprint profile received from the server.
   * The server generates the profile from the API key and sends it on auth_ok.
   * This guarantees every browser with the same API key gets the exact same
   * fingerprint, the server is the single source of truth.
   */
  async applyServerFingerprint(profile: FingerprintProfile, cookies: unknown[] = [], now?: number): Promise<void> {
    if (!profile?.id) return;
    // WebContents partitions are immutable. Rebuild views when the profile
    // changes so the tabs, listener and cookie exporter all use the same jar.
    const switched = this.active?.id !== profile.id;
    const reopen = switched ? this.leaveJar() : [];
    this.remember(profile);
    await this.enterJar(cookies, now, reopen);
    // Re-inject into all open tabs so they pick up the new fingerprint
    if (!switched) this.reprotectTabs();
    // Notify renderer so the fingerprint debug bar updates
    this.deps.shell.send('fingerprint-changed', fingerprintSummary(profile));
  }

  /** Closes every tab of the old jar and returns their addresses to reopen in the new one. */
  private leaveJar(): string[] {
    const tabs = this.deps.tabs.list;
    const reopen = tabs.map(addressOf);
    // Drop the pending batch rather than flushing it. Those changes were seen
    // under the previous persona, but this socket is already authenticated as
    // the new one, so the server would file another identity's cookies in this
    // persona's jar, planting a session captured on one device into a jar
    // that a different device will replay, which is what makes a site demand a
    // fresh login. The old jar keeps them; its partition is untouched.
    this.deps.cookies.dropPendingCookieChanges();
    for (const tab of [...tabs]) this.deps.tabs.closeTab(tab.id, { keepOne: false });
    this.deps.cookies.forgetPulls();
    return reopen;
  }

  /** Persist it so it survives restarts (and loads before reconnect). */
  private remember(profile: FingerprintProfile): void {
    if (this.store) {
      this.store.save(profile);
      this.store.setActiveId(profile.id);
    }
    this.active = profile;
    this.deps.config.values.activeProfileId = profile.id;
    this.deps.config.save();
  }

  /** Re-setup session with the new fingerprint (user-agent, proxy, headers), its cookies, and its tabs. */
  private async enterJar(cookies: unknown[], now: number | undefined, reopen: string[]): Promise<void> {
    await this.setupBrowserSession();
    this.deps.cookies.startCookieChangeListener();
    await this.deps.cookies.applyCookieSync(cookies, { now });
    for (const url of reopen) this.deps.tabs.createTab(url);
  }

  /** Same persona, fresh profile: every live tab gets it re-applied. */
  private reprotectTabs(): void {
    this.deps.workers?.cover();
    for (const tab of this.deps.tabs.list) {
      if (!tab.view.webContents.isDestroyed()) this.deps.protection.setupTabCDP(tab.view);
    }
  }
}
