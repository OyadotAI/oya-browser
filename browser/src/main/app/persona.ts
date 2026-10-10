/**
 * The persona this browser runs as: its fingerprint profile, the Electron
 * session (cookie jar) that belongs to it, and the localStorage transport for
 * its logins. The server is the single source of truth for the profile.
 */
import type { Session } from 'electron';
import type { AppServices } from './services.ts';
import { addressOf } from '../tabs/home.ts';
import type { Origins } from '../../page/login-state.ts';
import { ProfileStore } from '../../anonymity/profile-store.ts';
import { type SessionProfile } from '../identity/session.ts';
import { PersonaSessionSetup } from './persona-session.ts';
import { PersonaStorage } from '../sync/persona-storage.ts';
import { capturePersonaPolicy } from './persona-policy.ts';
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

/** Discover all native surfaces, including OAuth popups and background windows. */
function createPersonaStorage(deps: Deps): PersonaStorage {
  return new PersonaStorage({
    app: deps.electron.app,
    contents: () => deps.electron.webContents.getAllWebContents(),
    online: () => deps.socket.ready && deps.socket.isOpen(),
    send: (message) => deps.socket.send(message),
    report: (error) => deps.shell.send('profile-saved', { error }),
  });
}

/** The active persona and everything bound to it. */
export class Persona {
  /** The fingerprint profile in use, or null before the server sends one. */
  active: FingerprintProfile | null = null;
  /** Every identity transition invalidates asynchronous policy work, including switch-away-and-back. */
  policyEpoch = Symbol();
  /** Saved profiles on disk. */
  store: ProfileStore<FingerprintProfile> | null = null;
  /** Where a proxied persona's traffic comes out, as a timezone; null when direct or unknown. */
  exitZone: string | null = null;
  /** The main-process services. */
  private readonly deps: Deps;
  /** Owns immutable partition policy and excludes concurrent configuration. */
  private readonly sessionSetup: PersonaSessionSetup;
  /** Native session-bound login synchronization across tabs and popups. */
  private readonly storage: PersonaStorage;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.sessionSetup = new PersonaSessionSetup(this, deps);
    this.storage = createPersonaStorage(deps);
  }

  /** Release native observers only after final profile capture and local persistence are complete. */
  disposeStorage(): void {
    this.storage?.dispose();
  }

  /** Publish native snapshots before asking the server to persist this persona's profile. */
  flushStorage(): Promise<boolean> {
    return this.storage?.flush() || Promise.resolve(true);
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

  /** Configure this identity through an exclusive, generation-fenced native session owner. */
  setupBrowserSession(): Promise<void> {
    return this.sessionSetup.configure();
  }

  /** The fingerprint bar's view of the active profile, or null. */
  summary(): FingerprintSummary | null {
    if (!this.active) return null;
    return fingerprintSummary(this.active);
  }

  /** Bind synchronization to the authenticated native partition before importing any login state. */
  ensureLoginState(msg: LoginStateMessage): Promise<void> {
    return this.prepareNativeStorage(msg);
  }

  /** Resolve the authenticated identity explicitly; the old active persona must never receive its import. */
  private prepareNativeStorage(msg: LoginStateMessage): Promise<void> {
    if (!msg.fingerprint?.id) throw Error('Native profile storage requires an authenticated persona');
    const session = this.deps.electron.session.fromPartition(`persist:oya-${msg.fingerprint.id}`);
    return this.storage!.activate(session, msg.origins || {});
  }

  /**
   * Apply a fingerprint profile received from the server.
   * The server generates the profile from the API key and sends it on auth_ok.
   * This guarantees every browser with the same API key gets the exact same
   * fingerprint, the server is the single source of truth.
   */
  async applyServerFingerprint(profile: FingerprintProfile, cookies: unknown[] = [], now?: number): Promise<void> {
    if (!profile?.id) return;
    const epoch = (this.policyEpoch = Symbol());
    // WebContents partitions are immutable. Rebuild views when the profile
    // changes so the tabs, listener and cookie exporter all use the same jar.
    const switched = this.active?.id !== profile.id;
    const reopen = switched ? this.leaveJar() : [];
    await this.enterNativeProfile(profile, () => this.enterJar(cookies, now, reopen));
    this.publishProfile(epoch, profile, switched);
  }

  /** Never publish an activation superseded in the final promise continuation. */
  private publishProfile(epoch: symbol, profile: FingerprintProfile, switched: boolean): void {
    if (this.policyEpoch !== epoch) throw Error('Native persona changed during activation');
    if (!switched) this.reprotectTabs();
    this.deps.shell.send('fingerprint-changed', fingerprintSummary(profile));
  }

  /** Restore the previous identity if native setup refuses a changed or incomplete policy. */
  private async enterNativeProfile(profile: FingerprintProfile, enter: () => Promise<void>): Promise<void> {
    const previous = this.active;
    this.active = profile;
    const epoch = this.policyEpoch;
    const assertCurrent = capturePersonaPolicy(this, this.deps.governance);
    await this.attemptProfile(() => this.completeProfile(enter, assertCurrent, profile), epoch, previous);
  }

  /** Persist identity only after setup and its generation fence succeed. */
  private async completeProfile(
    enter: () => Promise<void>,
    assertCurrent: () => void,
    profile: FingerprintProfile,
  ): Promise<void> {
    await enter();
    assertCurrent();
    this.remember(profile);
  }

  /** Roll back only the identity still owned by this failed transition. */
  private async attemptProfile(enter: () => Promise<void>, epoch: symbol, prior: FingerprintProfile | null) {
    try {
      await enter();
    } catch (error) {
      this.restoreProfile(epoch, prior);
      throw error;
    }
  }

  /** A failed stale activation must never restore over a newer identity. */
  private restoreProfile(epoch: symbol, previous: FingerprintProfile | null): void {
    if (this.policyEpoch === epoch) this.active = previous;
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
    const assertCurrent = capturePersonaPolicy(this, this.deps.governance);
    await this.setupBrowserSession();
    assertCurrent();
    this.deps.cookies.startCookieChangeListener();
    await this.deps.cookies.applyCookieSync(cookies, { now });
    assertCurrent();
    for (const url of reopen) this.deps.tabs.createTab(url);
  }

  /** Same immutable persona: verify protection on every live surface. */
  private reprotectTabs(): void {
    this.deps.workers?.cover();
    for (const tab of this.deps.tabs.list) {
      if (!tab.view.webContents.isDestroyed()) this.deps.protection.setupTabCDP(tab.view);
    }
  }
}
