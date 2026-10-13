/**
 * Numbers and fixed values the app wiring (src/main/app/) runs on: file modes,
 * the saved config's shape, the defaults a fresh install starts from, the
 * updater's schedule and the random bytes the composition root draws.
 */

/**
 * 0600: owner read/write only. Config holds apiKey, a control-plane credential
 * for the whole project, and the default 0644 leaves a file readable by any
 * other local account on a Linux host or in a container.
 */
export const PRIVATE_FILE_MODE = 0o600;
/** Spaces per indent level in JSON written for people to read. */
export const JSON_INDENT = 2;
/** Where a fresh install looks for its workspace. A self-hoster edits the field. */
export const DEFAULT_SERVER_URL = 'wss://oyabrowser.com/ws';
/** Decimal places a fingerprint's noise seeds are shown with. */
export const NOISE_SEED_DIGITS = 6;
/** The scheme one-click sign-in links use. */
export const LINK_SCHEME = 'oya://';

/** How often a packaged app checks for an update (six hours). */
export const UPDATE_CHECK_INTERVAL_MS = 21_600_000;
/** The first check waits this long, so the first window can settle. */
export const UPDATE_FIRST_CHECK_MS = 10_000;

/** Random bytes in the CDP relay's token. */
export const RELAY_TOKEN_BYTES = 32;
/** Random bytes in the isolated world's name. */
export const WORLD_NAME_BYTES = 8;

/**
 * Pages keep rendering while the window is hidden or covered. An agent drives the
 * browser while the person works in other windows, and Chromium stops drawing a
 * window it thinks nobody sees: every click and keystroke then waited 30 seconds for
 * a frame, some ran past the command timeout, and those steps went missing from the
 * recording.
 */
export const KEEP_RENDERING_SWITCHES = [
  'disable-renderer-backgrounding',
  'disable-backgrounding-occluded-windows',
  'disable-background-timer-throttling',
] as const;

/** Bound URLs passed to the operating system's protocol handler. */
export const EXTERNAL_APP_URL_LIMIT = 8192;
/** Bound the installed application's display name in consent dialogs. */
export const EXTERNAL_APP_LABEL_LIMIT = 100;
/** The explicit affirmative button, never the dialog's default. */
export const OPEN_EXTERNAL_APP_CHOICE = 1;

/** Explicit approval, with Block as the default media permission response. */
export const ALLOW_MEDIA_CHOICE = 1;
/** The end-call dialog's "Close Window" or "Quit" button; Cancel is 0, the default. */
export const END_CALL_CHOICE = 1;
/** Past this many sites, the end-call dialog names the first and counts the rest. */
export const END_CALL_NAMED_SITES = 2;
/** How long after the OS announces a logout or power-off a quit still goes through unasked; past it, another app cancelled and calls are guarded again. */
export const SHUTDOWN_GRACE_MS = 60_000;
/** The file in the user-data folder the remembered site permissions live in. */
export const SITE_PERMISSIONS_FILE = 'site-permissions.json';
/** Bound native passkey chooser size and untrusted account labels. */
export const PASSKEY_ACCOUNT_LIMIT = 20;
/** Keep account buttons readable without control characters. */
export const PASSKEY_LABEL_LIMIT = 100;
/** Cancel abandoned account selection rather than retaining an authentication request indefinitely. */
export const PASSKEY_PROMPT_TIMEOUT_MS = 60_000;

/** Native chooser never makes credential use the default action. */
export const PASSKEY_DIALOG = { message: 'Sign in with a passkey', defaultId: 0, cancelId: 0, noLink: true };

/** Explicit development-only isolation test; never a production sign-in bypass. */
export const NATIVE_SIGNIN_TEST_SWITCH = '--oya-native-signin-test';
/** Same starting page for each fresh, human-driven compatibility experiment. */
export const NATIVE_SIGNIN_TEST_URL = 'https://mail.google.com/';
/** Native test window dimensions, independent of persona screen emulation. */
export const NATIVE_SIGNIN_TEST_SIZE = { width: 1100, height: 800 };
/** How many signed-out accounts the welcome screen offers to sign back in to. */
export const SAVED_ACCOUNT_LIMIT = 10;
/** Hex characters of a saved account's id: enough to tell keys apart, never the key itself. */
export const SAVED_ACCOUNT_ID_CHARS = 16;

/** The CDP version the passthrough attaches a tab's debugger with. */
export const PASSTHROUGH_CDP_VERSION = '1.3';
/** Page domains the external CDP front door passes through to a tab's own debugger when it has no native answer. */
export const PASSTHROUGH_DOMAINS = [
  'Accessibility',
  'Animation',
  'CSS',
  'DOM',
  'DOMSnapshot',
  'Emulation',
  'Fetch',
  'Input',
  'Inspector',
  'Log',
  'Network',
  'Overlay',
  'Page',
  'Performance',
  'Runtime',
  'Security',
];
/** Target methods a page session may pass through: its own iframes and workers, never other tabs. */
export const PASSTHROUGH_TARGET_METHODS = ['Target.setAutoAttach', 'Target.detachFromTarget'];
/** Passthrough methods refused: the persona owns identity, and the browser owns files and other tabs. */
export const PASSTHROUGH_REFUSED: Record<string, string> = {
  'Emulation.setUserAgentOverride': 'The persona owns the user agent',
  'Network.setUserAgentOverride': 'The persona owns the user agent',
  'Emulation.setTimezoneOverride': 'The persona owns the timezone',
  'Emulation.setLocaleOverride': 'The persona owns the locale',
  'Emulation.setHardwareConcurrencyOverride': 'The persona owns the device',
  'Page.setDownloadBehavior': 'Downloads follow this browser’s own policy',
  'DOM.setFileInputFiles': 'Files on the browser’s computer cannot be chosen remotely',
  'Page.close': 'Close tabs with Target.closeTarget, which keeps the browser’s tab ownership',
  'Page.crash': 'The browser’s renderers are not crashed remotely',
  'Network.loadNetworkResource': 'Load pages with Page.navigate, which checks the egress rules',
};
/** The browser's own profile as a CDP browserContextId: Playwright needs one on every target. */
export const DEFAULT_BROWSER_CONTEXT = 'default';
