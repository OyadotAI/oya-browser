/**
 * Numbers and fixed values the app wiring (main/app/) runs on: file modes,
 * the saved config's shape, and the defaults a fresh install starts from.
 */

/**
 * 0600: owner read/write only. Config holds apiKey, a control-plane credential
 * for the whole project, and the default 0644 leaves a file readable by any
 * other local account on a Linux host or in a container.
 */
const PRIVATE_FILE_MODE = 0o600;
/** Spaces per indent level in JSON written for people to read. */
const JSON_INDENT = 2;
/** Where a fresh install looks for its workspace. A self-hoster edits the field. */
const DEFAULT_SERVER_URL = 'wss://oyabrowser.com/ws';
/** Decimal places a fingerprint's noise seeds are shown with. */
const NOISE_SEED_DIGITS = 6;
/** The scheme one-click sign-in links use. */
const LINK_SCHEME = 'oya://';
/**
 * How long a signed-in launch waits for the server's cookies before opening its
 * pages anyway (offline). Opening them sooner let a site mint a logged-out cookie
 * that then replaced the logged-in copy the server held.
 */
const RESUME_OFFLINE_MS = 5000;

module.exports = {
  PRIVATE_FILE_MODE,
  JSON_INDENT,
  DEFAULT_SERVER_URL,
  NOISE_SEED_DIGITS,
  LINK_SCHEME,
  RESUME_OFFLINE_MS,
};
