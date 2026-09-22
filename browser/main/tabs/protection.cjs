/**
 * Protection for every page surface: the persona's fingerprint and stealth
 * injection, the dialog watcher, the login-state transport, and the isolated
 * world the analyzer runs in. Applied to tabs and to sign-in popups alike.
 */
const { buildInjectionScript } = require('../../anonymity/inject');
const { createPersonaApplier } = require('../../anonymity/apply');
const { CDP_VERSION, cdpAttach } = require('../cdp.cjs');
const { attachDialogWatcher } = require('../dialogs.cjs');
const { personaIdentity } = require('../identity.cjs');
const { normalizeProxy } = require('../../anonymity/proxy');
const governance = require('../../governance');

/**
 * A silent failure here would hide which tabs could not be protected. It is
 * said per attempt, not as the tab's verdict: the retry may still protect it,
 * and a tab that stays unprotected is never loaded (tab-events.cjs says so).
 */
const tabProtectionFailed = (what, err) => {
  console.error('[anonymity] ' + what + ' failed, this attempt did not protect the tab:', err?.message || err);
};

/** A step that failed on a tab whose protection held: worth a line, not an alarm. */
const tabStepFailed = (what, err) => {
  console.error('[anonymity] ' + what + ' failed on this tab:', err?.message || err);
};

/** The same failure, for a popup. */
const popupProtectionFailed = (what, e) => {
  console.error(`[anonymity] popup ${what} failed, popup is NOT protected:`, e?.message || e);
};

/** Electron has no passkey dialog and no permission prompt: the injection answers both as Chrome would (stealth.js). */
const DESKTOP_INJECTION = { noPasskeyDialog: true, noPermissionPrompt: true };
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
function atThisExit(profile) {
  const proxy = normalizeProxy(governance.configuration?.proxy || profile.proxy);
  if (proxy?.host) return profile;
  return { ...profile, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone };
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
function onThisMachine(profile, config) {
  if (config?.provider || process.env.OYA_DOCKER) return profile;
  return { ...profile, screen: null, canvas: { ...profile.canvas, noiseSeed: null } };
}

/** Subscribes `fn` to one CDP event on a debugger. */
const onDebuggerEvent = (dbg) => (method, fn) =>
  dbg.on('message', (_event, event, params) => {
    if (event === method) fn(params);
  });

/** No persona yet: Chrome's identity in place of Electron's, and the stealth injection. */
async function injectStealthOnly(port, userAgent, fail) {
  await port.send('Emulation.setUserAgentOverride', userAgent).catch((e) => fail('user agent override', e));
  const source = buildInjectionScript(null, DESKTOP_INJECTION);
  return port.send('Page.addScriptToEvaluateOnNewDocument', { source }).catch((e) => fail('stealth injection', e));
}

/** The persona applier's view of a debugger, sessions included. */
function personaPort(dbg) {
  return {
    send: (method, params, sessionId) => dbg.sendCommand(method, params, sessionId),
    on: (event, fn) => dbg.on('message', (_e, method, params, sessionId) => method === event && fn(params, sessionId)),
  };
}

/** A port for `x`: a debugger is wrapped, anything that already speaks send/on is one. */
const portOf = (x) => (typeof x.sendCommand === 'function' ? personaPort(x) : x);

/** Why a replaced attempt's commands never reach the debugger. */
const REPLACED = 'this protection attempt was replaced by a retry';

/**
 * A debugger port for one setup attempt. A retry detaches and attaches again,
 * and the old attempt, still running, would carry on sending on the new
 * session: each of its steps catches its error and sends the next, the
 * injection included. Fenced, it sends nothing and hears nothing once replaced,
 * so a page is never injected twice.
 */
function fencedPort(dbg, attempt) {
  const port = personaPort(dbg);
  return {
    send: (...args) => (attempt.live ? port.send(...args) : Promise.reject(new Error(REPLACED))),
    on: (event, fn) => port.on(event, (...args) => attempt.live && fn(...args)),
  };
}

/** A setup attempt's critical failure: it marks the attempt failed and says so, unless a retry already replaced it. */
const criticalIn = (attempt) => (what, err) => {
  if (!attempt.live) return;
  attempt.failed = true;
  tabProtectionFailed(what, err);
};

/** A view with no debugger to attach: it is not protected, and says so. */
function notAttached() {
  tabProtectionFailed('debugger attach', new Error('view destroyed'));
  return false;
}

/** The analyzer's world could not be rebuilt; the next command loads it. Quiet for a tab that has gone. */
function worldNotRebuilt(view, err) {
  if (view.webContents.isDestroyed?.()) return;
  console.error('[anonymity] isolated world not rebuilt, the next command loads it:', err?.message || err);
}

/** Applies the persona to tabs and popups. */
class Protection {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
  }

  /**
   * The persona on one webContents, applied as the server's CDP driver applies
   * it (anonymity/apply.js): emulation, the injection, and the tab's workers and
   * cross-site iframes. The screen stays the window's own. The user agent is
   * overridden here as well as on the session, because only the override
   * reaches navigator.userAgentData, which the session's string cannot.
   */
  applyPersona(dbg, fail) {
    const port = portOf(dbg);
    const active = this.ctx.persona.active;
    const userAgent = personaIdentity(active).override;
    if (!active) return injectStealthOnly(port, userAgent, fail);
    const profile = onThisMachine(atThisExit(active), this.ctx.config?.values);
    const applier = { ...port, ...DESKTOP_APPLIER, profile, userAgent, onError: fail };
    return createPersonaApplier(applier).page();
  }

  /**
   * Protects a tab's page, once per view, and answers whether it is protected.
   * Failures are logged loudly, never thrown; a view already being set up
   * answers what its attempt answers.
   */
  async setupTabCDP(view) {
    if (!cdpAttach(view)) return notAttached();
    // Main world: only what the page itself must see, as one script in one
    // scope so the toString mask covers the fingerprint patches too. The
    // analyzer is loaded separately into an isolated world by ensureWorld().
    // A second caller while an attempt runs (reprotecting every tab on a
    // reconnect) waits for that attempt's answer: an early true would let the
    // first page load before the injection landed.
    if (view.oyaConfigured) return view.oyaSetup;
    view.oyaConfigured = true;
    const attempt = (view.oyaAttempt = { live: true, failed: false });
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
  resetTabCDP(view) {
    if (view.oyaAttempt) view.oyaAttempt.live = false;
    view.oyaConfigured = false;
    this.ctx.recorder?.channels?.forget(view);
    try {
      view.webContents.debugger.detach();
    } catch {}
  }

  /** The persona, dialogs, login state and isolated world on one tab's debugger, all through one attempt's port. */
  async protectTab(view, attempt) {
    const dbg = view.webContents.debugger;
    const port = fencedPort(dbg, attempt);
    await this.applyPersona(port, criticalIn(attempt));
    port.send('Page.enable').catch((e) => tabStepFailed('Page.enable', e));
    attachDialogWatcher(dbg);
    const loginState = this.ctx.persona.loginState;
    if (loginState) await loginState.attach((method, params = {}) => port.send(method, params), port.on);
    this.rebuildWorldOnLoad(view);
  }

  /**
   * A fresh document means a fresh isolated world; rebuild it eagerly so the
   * first command after a navigation does not pay for it. Wired once per view,
   * whatever number of attempts it took. The world is the analyzer's, not the
   * tab's protection, so a failure here says only that, and nothing at all for
   * a tab that was closed mid-load.
   */
  rebuildWorldOnLoad(view) {
    if (view.oyaWorldWired) return;
    view.oyaWorldWired = true;
    view.webContents.on('did-finish-load', () => {
      this.ctx.world.ensureWorld(view, { force: true }).catch((e) => worldNotRebuilt(view, e));
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
  protectPopup(childWindow) {
    this.ctx.shield.adoptPopup(childWindow);
    try {
      this.protectPopupDebugger(childWindow.webContents.debugger);
    } catch (e) {
      console.error('[anonymity] popup debugger attach failed, popup is NOT protected:', e.message);
    }
  }

  /** The persona, dialogs and login state on a popup's debugger. */
  protectPopupDebugger(dbg) {
    if (!dbg.isAttached()) dbg.attach(CDP_VERSION);
    this.applyPersona(dbg, popupProtectionFailed);
    dbg.sendCommand('Page.enable').catch(() => {});
    attachDialogWatcher(dbg);
    this.syncPopupLogins(dbg);
  }

  /**
   * A sign-in popup is where the session actually gets written, so it needs
   * the same localStorage transport a tab gets. Without this the cookies
   * synced but the token half of a login stayed on this machine.
   */
  syncPopupLogins(dbg) {
    const loginState = this.ctx.persona.loginState;
    if (!loginState) return;
    loginState
      .attach((method, params = {}) => dbg.sendCommand(method, params), onDebuggerEvent(dbg))
      .catch((e) => console.error('[oya] popup login state not synced:', e.message));
  }

  /** Ensure the analyzer is loaded in this view's isolated world (fallback, CDP auto-inject is primary). */
  async injectScripts(view) {
    if (!view) view = this.ctx.tabs.getActiveView();
    if (!view) return;
    try {
      await this.ctx.world.ensureWorld(view);
    } catch (e) {
      console.error('[anonymity] isolated world unavailable, analyzer not loaded:', e.message);
    }
  }
}

module.exports = { Protection };
