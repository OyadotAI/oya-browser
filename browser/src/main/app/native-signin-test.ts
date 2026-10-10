/**
 * Human-only, development-only Oya compatibility experiment. No identity overrides,
 * preload, debugging transport, agents, or login sync. It does not promise Google
 * acceptance or supply the unfinished native passkey engine.
 */
import type { BrowserWindow, BrowserWindowConstructorOptions } from 'electron';
import { NATIVE_SIGNIN_TEST_SWITCH, NATIVE_SIGNIN_TEST_URL, NATIVE_SIGNIN_TEST_SIZE } from './constants.ts';

/** Refuse diagnostics where they could bypass a managed policy or expose debugging. */
export function nativeSigninTestEnabled(packaged: boolean, args: string[], env: NodeJS.ProcessEnv): boolean {
  if (!args.includes(NATIVE_SIGNIN_TEST_SWITCH) && !args.includes('--oya-native-browsing')) return false;
  if (packaged || env.OYA_GOVERNANCE || env.OYA_DOCKER || forbiddenListener(args, env))
    throw new Error('Native sign-in diagnostics require an unmanaged development launch without remote debugging.');
  if (args.some((arg) => /^--(?:remote-debugging|inspect|enable-automation)/.test(arg)))
    throw new Error('Native sign-in diagnostics cannot run with debugging or automation switches.');
  return true;
}

/** Human sign-in diagnostics prohibit all listeners; native browsing may use the authenticated compatibility alias. */
function forbiddenListener(args: string[], env: NodeJS.ProcessEnv): boolean {
  if (args.includes(NATIVE_SIGNIN_TEST_SWITCH)) return !!(env.OYA_REMOTE_DEBUGGING_PORT || env.OYA_NATIVE_CDP_PORT);
  return !!env.OYA_REMOTE_DEBUGGING_PORT && !env.OYA_NATIVE_CDP_TOKEN;
}

/** Native browser security defaults, without a persistent credential partition. */
const TEST_WEB_PREFERENCES = {
  partition: 'oya-native-signin-test',
  sandbox: true,
  contextIsolation: true,
  nodeIntegration: false,
  webSecurity: true,
  devTools: false,
};

/** Sandboxed, memory-only page with no Electron APIs exposed to website scripts. */
export function nativeSigninTestOptions(): BrowserWindowConstructorOptions {
  return {
    ...NATIVE_SIGNIN_TEST_SIZE,
    title: 'Oya — native sign-in diagnostic (not a passkey fix)',
    webPreferences: { ...TEST_WEB_PREFERENCES },
  };
}

/** Only normal secure website navigations belong in this diagnostic window. */
export function nativeSigninTestAllows(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'https:' && !parsed.username && !parsed.password;
  } catch {
    return false;
  }
}

/** Deny unsafe navigation without installing page scripts or overriding the engine. */
function guardNavigation(event: { preventDefault(): void }, url: string): void {
  if (!nativeSigninTestAllows(url)) event.preventDefault();
}

/** Keep the actual origin visible; reject popups rather than creating unconfigured surfaces. */
function protectTestWindow(window: BrowserWindow): void {
  const contents = window.webContents;
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
  contents.on('will-navigate', guardNavigation);
  contents.on('will-redirect', guardNavigation);
  contents.on('page-title-updated', (event) => event.preventDefault());
  contents.on('did-navigate', (_event, url) => window.setTitle(`Oya native diagnostic — ${new URL(url).origin}`));
}

/** Launch only the diagnostic surface; the normal boot, IPC and lifecycle must not run. */
export function startNativeSigninTest(electron: Pick<typeof import('electron'), 'BrowserWindow' | 'Menu'>): void {
  electron.Menu.setApplicationMenu(null);
  const window = new electron.BrowserWindow(nativeSigninTestOptions());
  window.webContents.session.setPermissionRequestHandler((_contents, _permission, answer) => answer(false));
  window.webContents.session.setPermissionCheckHandler(() => false);
  protectTestWindow(window);
  void window
    .loadURL(NATIVE_SIGNIN_TEST_URL)
    .catch(() => window.setTitle('Oya native diagnostic — page failed to load'));
}
