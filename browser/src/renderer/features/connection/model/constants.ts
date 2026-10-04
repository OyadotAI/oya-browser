/**
 * The connection feature's fixed values: the server-address rule, the overlay
 * names the main process knows, and the words the ViewModels say. Timeouts
 * and time units come from core/constants.ts.
 */

/**
 * A usable server address: wss:// anywhere, or plaintext ws:// only to this
 * machine, the rule pairing links follow (src/main/connection/pairing.ts). Anywhere else
 * ws:// would put the key and every synced cookie on the wire in the clear.
 */
export const SERVER_URL = /^(wss:\/\/[^\s]+|ws:\/\/(localhost|127\.0\.0\.1|\[::1\])(:\d+)?([/?#][^\s]*)?)$/i;

/** The overlay names the main process tracks (src/main/shell/overlays.ts); the reconnect dialog uses its default. */
export const OVERLAYS = { shell: 'shell', reconnect: 'legacy' } as const;

/** The browsers named in the empty import state before the ones on this computer are known. */
export const DEFAULT_IMPORT_BROWSERS = ['Chrome', 'Arc', 'Firefox'] as const;

/** What people call each platform a persona can claim. */
export const PLATFORM_NAMES: Readonly<Record<string, string>> = {
  Win32: 'Windows',
  MacIntel: 'Mac',
  'Linux x86_64': 'Linux',
};

/** The longest name a browser may have (the name field's maxlength). */
export const NAME_MAX_LENGTH = 64;

/** The words the ViewModels say. */
export const TEXT = {
  badServer: 'Enter a valid wss:// server address (ws:// only for this computer).',
  noKey: 'API key is required',
  saveFailed: 'Could not save connection settings.',
  loadFailed: 'Could not load connection settings.',
  badConsole: 'The server address is not valid. Fix it under "Connect with an API key".',
  setupTimedOut: 'Could not connect, check URL and API key',
  connect: 'Connect',
  connecting: 'Connecting...',
  reconnectInvalid: 'Enter a ws:// or wss:// server address and an API key.',
  reconnectIdle: 'Save & Reconnect',
  reconnectRetry: 'Retry connection',
  reconnectBusy: 'Connecting…',
  reconnectTimedOut: 'Could not connect. Check the server address and API key, then try again.',
  syncing: 'Syncing…',
  syncUnconfirmed: 'Sync not confirmed. Try again.',
  nameSaved: 'Name saved.',
  importDone: 'Done. You are signed in to those sites here now.',
  importEmpty: 'Nothing to import: that browser has no profiles with logins.',
  importNoBrowser: 'No supported browser found on this computer.',
  importOffline: 'Connect to Oya to import your logins.',
  restarting: 'Restarting…',
  noServer: 'Not configured',
  noBrowserId: 'Not connected',
} as const;
