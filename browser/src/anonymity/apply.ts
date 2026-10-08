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
 *
 * The server imports this file too, so it imports nothing but the page-side
 * injection (anonymity/inject.js).
 */
import { buildInjectionScript, buildWorkerScript } from '../../anonymity/inject.js';

/** The persona fields emulation reads; the rest of the profile goes to the injection. */
export interface PersonaProfile {
  /** The screen the persona reports; null where the host window owns the screen (`screen: false`). */
  screen: Partial<Record<'width' | 'height', number>> | null;
  /** The navigator the persona reports. */
  navigator: Partial<Record<'hardwareConcurrency', number>>;
  /** An IANA time zone, when the persona sets one. */
  timezone?: string | null;
  /** A locale, when the persona sets one. */
  locale?: string | null;
}

/** Sends one CDP command, to a session or (without one) the connection's own target. */
export type CdpSend = (method: string, params?: object, sessionId?: string) => Promise<unknown>;

/** Subscribes to one CDP event. */
export type CdpOn = (event: string, listener: (params: unknown) => void) => unknown;

/** A CDP event about a child target. */
export interface CdpAttachEvent {
  /** The child's session. */
  sessionId?: string;
  /** What the child is. */
  targetInfo?: Partial<Record<'type', string>>;
  /** Whether it is paused until told to run. */
  waitingForDebugger?: boolean;
}

/** Params for Emulation/Network.setUserAgentOverride. */
export interface UserAgentOverride {
  /** The user agent string; the override carries whatever else Chrome takes (platform, metadata, ...). */
  userAgent: string;
}

/** Options for the page injection (inject.js). */
export interface InjectionOptions {
  /** Direct, unmanaged desktop browsing may use native call connectivity. */
  nativeWebRTC?: boolean;
  /** The runtime cannot show a passkey dialog. */
  noPasskeyDialog?: boolean;
  /** The runtime refuses what Chrome would prompt for. */
  noPermissionPrompt?: boolean;
}

/** Called for a failure that leaves a surface unprotected. */
type OnError = (what: string, err: unknown) => void;

/** What one applier is built from. */
export interface ApplierOptions {
  /** The CDP transport's send. */
  send: CdpSend;
  /** The CDP transport's event subscription. */
  on: CdpOn;
  /** The anonymity profile. */
  profile: PersonaProfile;
  /** Params for Emulation/Network.setUserAgentOverride, or null to leave it. */
  userAgent?: UserAgentOverride | null;
  /** Emulate the screen (off where the host window owns it). */
  screen?: boolean;
  /** Options for the page injection. */
  injection?: InjectionOptions;
  /** (what, err), for failures that leave a surface unprotected. */
  onError?: OnError;
}

/** What every step of one applier shares: the transport, the persona and its built sources. */
interface ApplierContext {
  /** The CDP transport's send. */
  send: CdpSend;
  /** The anonymity profile. */
  profile: PersonaProfile;
  /** The user-agent override, or null. */
  userAgent: UserAgentOverride | null;
  /** Whether to emulate the screen. */
  screen: boolean;
  /** A catch handler that reports `what` failed. */
  report: (what: string) => (err: unknown) => void;
  /** Child sessions already covered. */
  covered: Set<string | undefined>;
  /** The page injection's source. */
  pageSource: string;
  /** The worker injection's source. */
  workerSource: string;
}

/** Which emulation to include. */
type EmulationOptions = Partial<Record<'screen', boolean>>;

/** One CDP command: its method and params. */
type Command = [string, object];

/** Sets up one newly attached child target. */
type Setup = (ctx: ApplierContext, sessionId?: string) => Promise<unknown>;

/** The persona applier: cover a page session, and the browser's own workers. */
export interface PersonaApplier {
  /** A page (or cross-site iframe) session. Undefined for a page-level debugger. */
  page(sessionId?: string): Promise<unknown>;
  /** Service and shared workers belong to the browser: needs a browser-level connection. */
  browser(): Promise<unknown>;
}

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
function screenOverride(profile: PersonaProfile): Command {
  if (!profile.screen) throw new Error('The persona has no screen to emulate');
  const params = { ...SCREEN_ONLY, screenWidth: profile.screen.width, screenHeight: profile.screen.height };
  return ['Emulation.setDeviceMetricsOverride', params];
}

/** Page-level emulation. Commands an older Chrome lacks fail quietly. */
export function emulationFor(profile: PersonaProfile, { screen = true }: EmulationOptions = {}): Command[] {
  const cmds: Command[] = [
    ['Emulation.setAutomationOverride', { enabled: false }],
    ['Emulation.setHardwareConcurrencyOverride', { hardwareConcurrency: profile.navigator.hardwareConcurrency }],
  ];
  if (profile.timezone) cmds.push(['Emulation.setTimezoneOverride', { timezoneId: profile.timezone }]);
  if (profile.locale) cmds.push(['Emulation.setLocaleOverride', { locale: profile.locale }]);
  if (screen) cmds.push(screenOverride(profile));
  return cmds;
}

/** The user agent, then native emulation; emulation a Chrome lacks fails quietly. */
async function emulate(ctx: ApplierContext, sessionId?: string): Promise<void> {
  if (ctx.userAgent) {
    await ctx.send('Emulation.setUserAgentOverride', ctx.userAgent, sessionId).catch(ctx.report('user agent override'));
  }
  for (const [method, params] of emulationFor(ctx.profile, { screen: ctx.screen })) {
    await ctx.send(method, params, sessionId).catch(() => {});
  }
}

/** Emulation, the injection, then coverage of the page's own child targets. */
async function applyToPage(ctx: ApplierContext, sessionId?: string): Promise<void> {
  const { send, report } = ctx;
  await emulate(ctx, sessionId);
  await send('Page.addScriptToEvaluateOnNewDocument', { source: ctx.pageSource }, sessionId).catch(report('injection'));
  const autoAttach = { autoAttach: true, waitForDebuggerOnStart: true, flatten: true, filter: PAGE_CHILDREN };
  await send('Target.setAutoAttach', autoAttach, sessionId).catch(report('child target coverage'));
}

/** A worker has no new-document hook: evaluate the persona before its script runs. */
async function applyToWorker(ctx: ApplierContext, sessionId?: string): Promise<void> {
  if (ctx.userAgent) await ctx.send('Network.setUserAgentOverride', ctx.userAgent, sessionId).catch(() => {});
  await ctx.send('Runtime.evaluate', { expression: ctx.workerSource }, sessionId).catch(ctx.report('worker injection'));
}

/** Which setup a newly attached target needs: a page's, a worker's, or none. */
function setupFor(type: string): Setup | null {
  if (type === 'iframe') return applyToPage;
  return type.endsWith('worker') ? applyToWorker : null;
}

/** A Target event's params as the transport delivers them: an object, or nothing. */
const attachEvent = (params: unknown): CdpAttachEvent => (params && typeof params === 'object' ? params : {});

/** A child target attached: cover it once, then let it run. */
function onAttached(ctx: ApplierContext, { sessionId, targetInfo, waitingForDebugger }: CdpAttachEvent): void {
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
function applierContext(options: ApplierOptions & Required<Pick<ApplierOptions, 'onError'>>): ApplierContext {
  const { send, profile, userAgent = null, screen = true, injection, onError } = options;
  const report = (what: string) => (err: unknown) => onError(what, err);
  const pageSource = buildInjectionScript(profile, injection);
  const workerSource = buildWorkerScript(profile, { ...injection, userAgent: userAgent?.userAgent || null });
  return { send, profile, userAgent, screen, report, covered: new Set(), pageSource, workerSource };
}

/** Builds the applier for one connection and starts covering child targets. */
export function createPersonaApplier({ onError = () => {}, ...options }: ApplierOptions): PersonaApplier {
  const ctx = applierContext({ ...options, onError });
  options.on('Target.attachedToTarget', (event) => onAttached(ctx, attachEvent(event)));
  options.on('Target.detachedFromTarget', (event) => ctx.covered.delete(attachEvent(event).sessionId));
  return {
    page: (sessionId) => applyToPage(ctx, sessionId),
    browser: () =>
      options.send('Target.setAutoAttach', BROWSER_AUTO_ATTACH).catch(ctx.report('service worker coverage')),
  };
}
