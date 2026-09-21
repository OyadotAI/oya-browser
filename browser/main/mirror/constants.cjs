/**
 * Every number the mirror import runs on, by name: how long to wait for the
 * launched browser and its CDP replies, how much to read, and the files that
 * carry a login. Kept here so no step hides a magic number.
 */

/** Milliseconds to wait for the launched browser's debugging port to answer. */
const LAUNCH_READY_TIMEOUT_MS = 20_000;
/** Milliseconds between polls of the debugging port while it comes up. */
const LAUNCH_POLL_MS = 200;
/** Milliseconds to wait for one CDP command's reply before giving up. */
const CDP_COMMAND_TIMEOUT_MS = 15_000;
/** Milliseconds to wait for the server to answer an import before telling the person it did not. */
const MIRROR_ANSWER_TIMEOUT_MS = 30_000;
/** Origins whose localStorage the capture reads, at most; bounds a huge profile. */
const MAX_CAPTURE_ORIGINS = 60;
/** HTTP status the capture fulfils a page request with, so its scripts never run. */
const FULFILL_STATUS = 200;
/** The token a headless Chrome puts in its user agent, stripped so the UA reads as real. */
const HEADLESS_TOKEN = /HeadlessChrome/g;
/** Milliseconds per second: Firefox stores cookie expiry in milliseconds, the pool in seconds. */
const MS_PER_SECOND = 1000;
/** Bytes the sqlite3 cookie dump may return (128 MiB); a full Firefox jar is over the 1 MiB default. */
const SQLITE_MAX_BUFFER = 134_217_728;

/**
 * The login-bearing files copied out of a real profile. Cookies decrypt over
 * CDP; these carry the rest of a session (localStorage, IndexedDB, service
 * workers) and are copied as files, since that is how Chromium stores them.
 */
const PROFILE_STORES = ['Local Storage', 'IndexedDB', 'Service Worker', 'WebStorage', 'File System'];

/** The files the launched browser needs to open the profile, copied before launch. */
const LAUNCH_INPUTS = ['Local State'];
/** Per-profile files the launched browser needs; joined under the profile dir. Never `Login Data`: saved passwords play no part in reading cookies, so they are not copied anywhere. */
const PROFILE_INPUTS = ['Cookies', 'Network', 'Preferences'];

module.exports = {
  MS_PER_SECOND,
  SQLITE_MAX_BUFFER,
  LAUNCH_READY_TIMEOUT_MS,
  LAUNCH_POLL_MS,
  CDP_COMMAND_TIMEOUT_MS,
  MIRROR_ANSWER_TIMEOUT_MS,
  MAX_CAPTURE_ORIGINS,
  FULFILL_STATUS,
  HEADLESS_TOKEN,
  PROFILE_STORES,
  LAUNCH_INPUTS,
  PROFILE_INPUTS,
};
