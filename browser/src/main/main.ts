/**
 * Oya Browser, Desktop browser with agent scripts built in.
 * Users run this on their machines. Connects to a deployed Oya server.
 * Multi-tab, persistent cookies, real browser, no extension install needed.
 *
 * All user input (click, type, key press, scroll) goes through Chrome DevTools
 * Protocol for full native control. Human-like timing and mouse paths.
 *
 * This file is the composition root and the only one that imports Electron at
 * runtime: it sets the process-wide flags that must precede `ready`, builds
 * each main-process service once into `ctx` (src/main/app/services.ts), and
 * wires Electron's events to them. See ARCHITECTURE.md.
 */
import electron, { type BrowserView } from 'electron';
import path from 'node:path';
import fs from 'node:fs';
import crypto from 'node:crypto';
import { applyTelemetryFlags } from '../anonymity/telemetry.ts';
import { applyDNSLeakPrevention } from '../anonymity/proxy.ts';
import { startAppNativeCdp } from './app/native-cdp.ts';
import { nativeSigninTestEnabled, startNativeSigninTest } from './app/native-signin-test.ts';
import { Passkeys, configurePasskeys } from './app/passkeys.ts';
/** Signing-bound WebAuthn group embedded by the build, empty for unsigned development. */
declare const __OYA_WEBAUTHN_GROUP__: string;
import { MediaPermissions } from './app/media-permissions.ts';
import { ExternalApps } from './app/external-apps.ts';
import { Notifications } from './notifications/index.ts';
import type { AppServices } from './app/services.ts';
import { ConfigStore } from './app/config-store.ts';
import { Persona } from './app/persona.ts';
import { DeepLinks } from './app/deep-links.ts';
import { Boot } from './app/boot.ts';
import { Lifecycle } from './app/lifecycle.ts';
import { Updater } from './app/updater.ts';
import { KEEP_RENDERING_SWITCHES, RELAY_TOKEN_BYTES, WORLD_NAME_BYTES } from './app/constants.ts';
import { DesktopDialogs, dialogPresenter } from './dialogs/index.ts';
import { World } from './native/index.ts';
import { Observer } from './observe/observer.ts';
import { DesktopControl } from './control/control-state.ts';
import { CookieSync, cookieSyncMark } from './sync/cookie-sync.ts';
import { Routines } from './routines/routines.ts';
import { PageDriver } from './actions/driver.ts';
import { Keyboard } from './input/keyboard.ts';
import { Mouse } from './input/mouse.ts';
import { LiveStream } from './connection/stream.ts';
import { CdpRelay } from './connection/cdp-relay.ts';
import { ControlSocket } from './connection/socket.ts';
import { CommandRunner } from './connection/commands.ts';
import { Mirror } from './mirror/mirror.ts';
import { BrowserWindows } from './windows/index.ts';
import { type AnalysisResult } from './shell/control-shield.ts';
import type { ActionParams } from './shell/narration.ts';
import { BrowsingLibrary } from './library/index.ts';
import { Protection } from './tabs/protection.ts';
import { WorkerCoverage } from './tabs/workers.ts';
import { Recorder } from './recording/recorder.ts';
import { registerIpc } from './ipc/index.ts';
import { Governance, readGovernance } from './identity/governance.ts';

// First, as governance.js's import was: a malformed OYA_GOVERNANCE stops the browser before anything starts.
const governance = new Governance(readGovernance(process.env.OYA_GOVERNANCE));

const { app, nativeImage } = electron;
const NATIVE_BROWSING = nativeSigninTestEnabled(app.isPackaged, process.argv, process.env);
const NATIVE_SIGNIN_TEST = NATIVE_BROWSING && process.argv.includes('--oya-native-signin-test');
// Branding must not move existing cookies, profiles, or saved settings.
const desktopUserDataPath = app.getPath('userData');
app.setName('Oya Browser');
app.setPath('userData', desktopUserDataPath);

if (process.env.OYA_USER_DATA_DIR) app.setPath('userData', path.resolve(process.env.OYA_USER_DATA_DIR));
if (NATIVE_SIGNIN_TEST) app.setPath('userData', fs.mkdtempSync(path.join(app.getPath('temp'), 'oya-native-signin-')));
// CDP for automation harnesses. Off unless asked for: whoever reaches this port
// owns the browser. Chromium listens one port up on loopback; the CDP front door
// (src/main/front-door/) owns the public port and shows harnesses only real,
// protected tabs. No remote-allow-origins, CDP clients send no Origin, and
// allowing one would let any web page on the machine drive it.
const CDP_PORT = Number(process.env.OYA_REMOTE_DEBUGGING_PORT) || 0;
const CDP_RELAY_TOKEN = crypto.randomBytes(RELAY_TOKEN_BYTES).toString('hex');
if (!NATIVE_BROWSING) app.commandLine.appendSwitch('remote-debugging-port', CDP_PORT ? String(CDP_PORT + 1) : '0');

// Prevent crashes from unhandled errors
process.on('uncaughtException', (err) => {
  console.error('[oya] Uncaught exception:', err.message);
});
process.on('unhandledRejection', (reason) => {
  console.error('[oya] Unhandled rejection:', (reason as Error | undefined)?.message || reason);
});

// Apply telemetry + DNS leak prevention flags before app is ready
if (!NATIVE_BROWSING) {
  applyTelemetryFlags(app);
  applyDNSLeakPrevention(app);
}

// Pages keep rendering while the window is hidden or covered (see KEEP_RENDERING_SWITCHES).
for (const name of KEEP_RENDERING_SWITCHES) app.commandLine.appendSwitch(name);

// Set dock icon on macOS (needed for dev mode, built app uses icon from package.json)
//
// icon_1024, not icon.png: the latter is the 6250x6250 master electron-builder
// resizes at build time. Handing it to nativeImage decodes 156MB per
// representation, and macOS makes several, it was 596MB of CG image data in
// the main process, most of this app's memory, for a dock icon.
/** The app's folder: the package root in development, the asar when packaged. Main runs bundled from out/main/. */
const APP_DIR = app.getAppPath();
if (process.platform === 'darwin') {
  const iconPath = path.join(APP_DIR, 'build', 'icon_1024.png');
  if (fs.existsSync(iconPath)) app.whenReady().then(() => app.dock?.setIcon(nativeImage.createFromPath(iconPath)));
}

// ─── The services, built once ───

const analyzerScript = fs.readFileSync(path.join(APP_DIR, 'scripts', 'analyzer.js'), 'utf8');
const ISOLATED_WORLD = 'w' + crypto.randomBytes(WORLD_NAME_BYTES).toString('hex');

// Every service, by name; each one reaches the others through it. The values
// come first; each service is added in turn, in the order the next one needs it.
const ctx = {
  electron,
  nativeBrowsing: NATIVE_BROWSING,
  appDir: APP_DIR,
  analyzerScript,
  isolatedWorld: ISOLATED_WORLD,
  cdpPort: CDP_PORT,
  relayToken: CDP_RELAY_TOKEN,
  // What the page said and fetched, collected in this process so an agent can read
  // it back without a CDP domain the page could detect (see src/main/observe/).
  observer: new Observer(),
  governance,
  workspace: null,
  chatAbort: null,
  validationTabs: undefined,
} as AppServices;
ctx.config = new ConfigStore({ dir: () => app.getPath('userData'), safe: electron.safeStorage });
ctx.windows = new BrowserWindows(ctx);
ctx.passkeys = new Passkeys(ctx);
ctx.mediaPermissions = new MediaPermissions(ctx);
ctx.externalApps = new ExternalApps(ctx);
ctx.notifications = new Notifications({
  partition: () => ctx.persona.partitionName(),
  changed: () => ctx.shell.send('notifications-changed', null),
});
ctx.dialogs = new DesktopDialogs(
  () => ctx.control.snapshot().interactive,
  dialogPresenter(ctx),
  (message, url) => ctx.notifications.add(message, url),
);
ctx.protection = new Protection(ctx);
ctx.workers = new WorkerCoverage(ctx, () => app.getPath('userData'));
ctx.persona = new Persona(ctx);
app.once('will-quit', () => ctx.persona.disposeStorage());
ctx.library = new BrowsingLibrary(ctx);
ctx.recorder = new Recorder(ctx);
ctx.socket = new ControlSocket(ctx);
ctx.commands = new CommandRunner(ctx);
ctx.deepLinks = new DeepLinks(ctx);
ctx.routines = new Routines(ctx);
ctx.world = new World({ analyzerScript, worldName: ISOLATED_WORLD });
ctx.control = new DesktopControl({
  send: (message) => ctx.socket.send(message),
  changed: (state) => {
    ctx.dialogs.controlChanged();
    ctx.shield.controlChanged(state);
  },
});
ctx.cookies = new CookieSync({
  mark: cookieSyncMark(ctx),
  session: () => ctx.persona.session(),
  send: (payload) => ctx.socket.send(payload),
  open: () => ctx.socket.isOpen(),
  ready: () => ctx.socket.ready,
});
ctx.relay = new CdpRelay(ctx);
ctx.mirror = new Mirror(ctx);
ctx.stream = new LiveStream(ctx);
ctx.actions = new PageDriver({
  config: ctx.config,
  navigate: (url) => ctx.tabs.navigateActive(url),
  getActiveView: () => ctx.tabs.getActiveView(),
  pullCookiesFor: (url) => ctx.cookies.pullCookiesFor(url),
  injectScripts: (view) => ctx.protection.injectScripts(view),
  worldEval: <T>(view: Parameters<World['evaluate']>[0], expression: string) => ctx.world.evaluate<T>(view, expression),
  sendResult: (id, ok, data, error, code) => ctx.commands.sendResult(id, ok, data, error, code),
  createTab: (url, activate) => ctx.tabs.createTab(url, activate),
  closeTab: (id) => ctx.tabs.closeTab(id),
  requireHumanControl: () => ctx.shield.requireHumanControl(),
  // The page a command drives is a tab's BrowserView.
  analysisStarted: (view) => ctx.shield.analysisStarted(view as BrowserView),
  analysisFinished: (view, raw) => ctx.shield.analysisFinished(view as BrowserView, raw as AnalysisResult | null),
  narrate: (view, action, params) =>
    ctx.shield.acting(view as BrowserView, action, params as ActionParams | undefined).catch(() => {}),
  tabs: () => ctx.tabs.list,
  activeTabId: () => ctx.tabs.activeTabId,
  keyboard: new Keyboard(process.platform),
  mouse: new Mouse(),
});

// ─── Events ───

// Windows and Linux deliver the link as an argv entry to a second launch;
// without the single-instance lock that launch becomes a second browser with
// its own session, and the cookies land in the wrong place.
if (NATIVE_SIGNIN_TEST) {
  app.whenReady().then(() => startNativeSigninTest(electron));
  app.on('window-all-closed', () => app.quit());
} else {
  if (!app.requestSingleInstanceLock()) app.quit();
  else app.on('second-instance', (_event, argv) => ctx.deepLinks.onSecondInstance(argv));
  app.on('open-url', (event, url) => ctx.deepLinks.onOpenUrl(event, url));

  const lifecycle = new Lifecycle(ctx);
  ctx.updater = new Updater({ electron, shell: ctx.shell, beforeInstall: () => lifecycle.flushJar() });
  registerIpc(ctx);

  app.whenReady().then(async () => {
    configurePasskeys(app, __OYA_WEBAUTHN_GROUP__);
    await new Boot(ctx).run();
    startAppNativeCdp(ctx);
  });
  lifecycle.install();
}
