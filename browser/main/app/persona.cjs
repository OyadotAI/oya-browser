/**
 * The persona this browser runs as: its fingerprint profile, the Electron
 * session (cookie jar) that belongs to it, and the localStorage transport for
 * its logins. The server is the single source of truth for the profile.
 */
const { LoginState } = require('../../login-state');
const { ProfileStore } = require('../../anonymity/profile-store');
const { configureSession } = require('../session.cjs');
const { NOISE_SEED_DIGITS } = require('./constants.cjs');

/** What the fingerprint debug bar shows for a profile. */
function fingerprintSummary(profile) {
  return { id: profile.id, ...deviceSummary(profile), ...localeSummary(profile) };
}

/** The hardware a profile claims. */
function deviceSummary(profile) {
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
const noiseShown = (seed) => (typeof seed === 'number' ? seed.toFixed(NOISE_SEED_DIGITS) : 'real');

/**
 * Where a profile claims to be, and its noise seeds. A mirrored device carries no
 * font list and no noise: reading them unguarded threw inside auth_ok, which closed
 * the connection for good the moment an import succeeded.
 */
function localeSummary(profile) {
  return {
    timezone: profile.timezone,
    locale: profile.locale,
    fonts: profile.fonts?.available?.length || 0,
    canvasNoise: noiseShown(profile.canvas?.noiseSeed),
    audioNoise: noiseShown(profile.audio?.noiseSeed),
  };
}

/** The active persona and everything bound to it. */
class Persona {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** The fingerprint profile in use, or null before the server sends one. */
    this.active = null;
    /** Saved profiles on disk. */
    this.store = null;
    /** localStorage sync for this persona's logins, once the server has authenticated us. */
    this.loginState = null;
  }

  /** Loads the saved profile store and the profile last in use. */
  loadActive(userDataPath) {
    this.store = new ProfileStore(userDataPath);
    const activeId = this.ctx.config.values.activeProfileId || this.store.getActiveId();
    if (activeId) this.active = this.store.get(activeId);
  }

  /** Each persona has its own partition, so its cookies never mix with another's. */
  partitionName() {
    if (this.active) return `persist:oya-${this.active.id}`;
    return 'persist:oya-browser';
  }

  /** The Electron session for the active persona. */
  session() {
    return this.ctx.electron.session.fromPartition(this.partitionName());
  }

  /** Configure the persistent browser session, user-agent, cookies, privacy. */
  async setupBrowserSession() {
    await configureSession(this.session(), this.active, this.ctx.observer);
  }

  /** The fingerprint bar's view of the active profile, or null. */
  summary() {
    if (!this.active) return null;
    return fingerprintSummary(this.active);
  }

  /** A new LoginState for a new persona; the same persona keeps the one it has. */
  ensureLoginState(msg) {
    if (this.loginState && this.active?.id === msg.fingerprint?.id) return;
    this.loginState = new LoginState(msg.origins || {}, (origins) => {
      const socket = this.ctx.socket;
      if (socket.ready && socket.isOpen()) socket.send({ type: 'storage_changed', origins });
    });
  }

  /**
   * Apply a fingerprint profile received from the server.
   * The server generates the profile from the API key and sends it on auth_ok.
   * This guarantees every browser with the same API key gets the exact same
   * fingerprint, the server is the single source of truth.
   */
  async applyServerFingerprint(profile, cookies = [], now = undefined) {
    if (!profile?.id) return;
    // WebContents partitions are immutable. Rebuild views when the profile
    // changes so the tabs, listener and cookie exporter all use the same jar.
    const switched = this.active?.id !== profile.id;
    const reopen = switched ? this.leaveJar() : [];
    this.remember(profile);
    await this.enterJar({ cookies, now }, reopen);
    // Re-inject into all open tabs so they pick up the new fingerprint
    if (!switched) this.reprotectTabs();
    // Notify renderer so the fingerprint debug bar updates
    this.ctx.shell.send('fingerprint-changed', fingerprintSummary(profile));
  }

  /** Closes every tab of the old jar and returns their addresses to reopen in the new one. */
  leaveJar() {
    const tabs = this.ctx.tabs.list;
    const reopen = tabs.map((tab) => tab.url || 'about:blank');
    // Drop the pending batch rather than flushing it. Those changes were seen
    // under the previous persona, but this socket is already authenticated as
    // the new one, so the server would file another identity's cookies in this
    // persona's jar, planting a session captured on one device into a jar
    // that a different device will replay, which is what makes a site demand a
    // fresh login. The old jar keeps them; its partition is untouched.
    this.ctx.cookies.dropPendingCookieChanges();
    while (tabs.length) this.ctx.tabs.closeTab(tabs[0].id, { keepOne: false });
    this.ctx.cookies.forgetPulls();
    return reopen;
  }

  /** Persist it so it survives restarts (and loads before reconnect). */
  remember(profile) {
    if (this.store) {
      this.store.save(profile);
      this.store.setActiveId(profile.id);
    }
    this.active = profile;
    this.ctx.config.values.activeProfileId = profile.id;
    this.ctx.config.save();
  }

  /** Re-setup session with the new fingerprint (user-agent, proxy, headers), its cookies, and its tabs. */
  async enterJar({ cookies, now }, reopen) {
    await this.setupBrowserSession();
    this.ctx.cookies.startCookieChangeListener();
    await this.ctx.cookies.applyCookieSync(cookies, { now });
    for (const url of reopen) this.ctx.tabs.createTab(url);
  }

  /** Same persona, fresh profile: every live tab gets it re-applied. */
  reprotectTabs() {
    for (const tab of this.ctx.tabs.list) {
      if (!tab.view.webContents.isDestroyed()) this.ctx.protection.setupTabCDP(tab.view);
    }
  }
}

module.exports = { Persona, fingerprintSummary };
