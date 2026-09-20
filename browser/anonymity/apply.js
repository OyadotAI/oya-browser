/**
 * Apply a persona to a Chrome target over CDP, the same way in every runtime
 * (the CDP driver, the stealth harness, the desktop app).
 *
 * Order matters:
 *   1. Native emulation. A value Chrome reports itself cannot be caught lying.
 *   2. The injection, for what emulation cannot reach (stealth.js, fingerprint.js).
 *   3. Every child target. Dedicated workers, cross-site iframes, and service
 *      and shared workers are separate targets that
 *      addScriptToEvaluateOnNewDocument never reaches, so a detector reads the
 *      real machine there: CreepJS compares its service worker against the
 *      page, and CAPTCHA and Turnstile widgets live in cross-site iframes.
 *      Each starts paused, gets the persona, then runs.
 *
 * Transport-agnostic: send(method, params, sessionId) → Promise, and
 * on(event, (params, sessionId) => void).
 */

const { buildInjectionScript, buildWorkerScript } = require('./inject');

/** Child targets a page auto-attaches to: its dedicated workers and cross-site iframes. */
const PAGE_CHILDREN = [{ type: 'worker' }, { type: 'iframe' }, { exclude: true }];
/** Targets that belong to the browser rather than a page: service and shared workers. */
const BROWSER_WORKERS = [{ type: 'service_worker' }, { type: 'shared_worker' }, { exclude: true }];
/** Auto-attach, paused, to every service and shared worker the browser starts. */
const BROWSER_AUTO_ATTACH = { autoAttach: true, waitForDebuggerOnStart: true, flatten: true, filter: BROWSER_WORKERS };

/**
 * The screen only: width, height and deviceScaleFactor 0 leave the viewport
 * and pixel ratio alone.
 */
const SCREEN_ONLY = { width: 0, height: 0, deviceScaleFactor: 0, mobile: false };

/** The device-metrics override that sets the persona's screen size and nothing else. */
function screenOverride(profile) {
  const params = { ...SCREEN_ONLY, screenWidth: profile.screen.width, screenHeight: profile.screen.height };
  return ['Emulation.setDeviceMetricsOverride', params];
}

/** Page-level emulation. Commands an older Chrome lacks fail quietly. */
function emulationFor(profile, { screen = true } = {}) {
  const cmds = [
    ['Emulation.setAutomationOverride', { enabled: false }],
    ['Emulation.setHardwareConcurrencyOverride', { hardwareConcurrency: profile.navigator.hardwareConcurrency }],
  ];
  if (profile.timezone) cmds.push(['Emulation.setTimezoneOverride', { timezoneId: profile.timezone }]);
  if (profile.locale) cmds.push(['Emulation.setLocaleOverride', { locale: profile.locale }]);
  if (screen) cmds.push(screenOverride(profile));
  return cmds;
}

/** The user agent, then native emulation; emulation a Chrome lacks fails quietly. */
async function emulate(ctx, sessionId) {
  if (ctx.userAgent) {
    await ctx.send('Emulation.setUserAgentOverride', ctx.userAgent, sessionId).catch(ctx.report('user agent override'));
  }
  for (const [method, params] of emulationFor(ctx.profile, { screen: ctx.screen })) {
    await ctx.send(method, params, sessionId).catch(() => {});
  }
}

/** Emulation, the injection, then coverage of the page's own child targets. */
async function applyToPage(ctx, sessionId) {
  const { send, report } = ctx;
  await emulate(ctx, sessionId);
  await send('Page.addScriptToEvaluateOnNewDocument', { source: ctx.pageSource }, sessionId).catch(report('injection'));
  const autoAttach = { autoAttach: true, waitForDebuggerOnStart: true, flatten: true, filter: PAGE_CHILDREN };
  await send('Target.setAutoAttach', autoAttach, sessionId).catch(report('child target coverage'));
}

/** A worker has no new-document hook: evaluate the persona before its script runs. */
async function applyToWorker(ctx, sessionId) {
  if (ctx.userAgent) await ctx.send('Network.setUserAgentOverride', ctx.userAgent, sessionId).catch(() => {});
  await ctx.send('Runtime.evaluate', { expression: ctx.workerSource }, sessionId).catch(ctx.report('worker injection'));
}

/** Which setup a newly attached target needs: a page's, a worker's, or none. */
function setupFor(type) {
  if (type === 'iframe') return applyToPage;
  return type.endsWith('worker') ? applyToWorker : null;
}

/** A child target attached: cover it once, then let it run. */
function onAttached(ctx, { sessionId, targetInfo, waitingForDebugger } = {}) {
  const type = targetInfo?.type || '';
  const setup = setupFor(type);
  const first = setup && !ctx.covered.has(sessionId);
  if (first) ctx.covered.add(sessionId);
  const resume = () => waitingForDebugger && ctx.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => {});
  // Whatever happened in setup, a paused target must run: a stuck worker breaks the site.
  Promise.resolve(first ? setup(ctx, sessionId) : null)
    .catch(ctx.report(`${type} setup`))
    .finally(resume);
}

/** What every step of one applier shares: the transport, the persona and its built sources. */
function applierContext({ send, profile, userAgent, screen, onError }) {
  const report = (what) => (err) => onError(what, err);
  const ctx = { send, profile, userAgent, screen, report, covered: new Set() };
  ctx.pageSource = buildInjectionScript(profile);
  ctx.workerSource = buildWorkerScript(profile, { userAgent: userAgent?.userAgent || null });
  return ctx;
}

/**
 * Builds the applier for one connection and starts covering child targets.
 * @param {object} o
 * @param {Function} o.send
 * @param {Function} o.on
 * @param {object} o.profile - anonymity profile
 * @param {object|null} [o.userAgent] - params for Emulation/Network.setUserAgentOverride
 * @param {boolean} [o.screen] - emulate the screen (off where the host window owns it)
 * @param {Function} [o.onError] - (what, err), for failures that leave a surface unprotected
 */
function createPersonaApplier({ send, on, profile, userAgent = null, screen = true, onError = () => {} }) {
  const ctx = applierContext({ send, profile, userAgent, screen, onError });
  on('Target.attachedToTarget', (event) => onAttached(ctx, event));
  on('Target.detachedFromTarget', ({ sessionId } = {}) => ctx.covered.delete(sessionId));
  return {
    /** A page (or cross-site iframe) session. Undefined for a page-level debugger. */
    page: (sessionId) => applyToPage(ctx, sessionId),
    /** Service and shared workers belong to the browser: needs a browser-level connection. */
    browser: () => send('Target.setAutoAttach', BROWSER_AUTO_ATTACH).catch(ctx.report('service worker coverage')),
  };
}

module.exports = { createPersonaApplier, emulationFor };
