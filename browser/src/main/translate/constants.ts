/** Numbers page translation (src/main/translate/) runs on. */

/** The isolated world translation's page scripts run in: 1004 is the recorder's, 1005 the picker's. */
export const TRANSLATE_WORLD_ID = 1006;

/** Most characters of one page sent to be translated; past it the page is translated in part, and says so. */
export const TRANSLATE_PAGE_CHARS = 20_000;

/** Most characters in one request: the server's TRANSLATE_BATCH_CHARS, which refuses more. */
export const TRANSLATE_BATCH_CHARS = 4_000;

/** Most text nodes in one request: the server's TRANSLATE_BATCH_SEGMENTS. */
export const TRANSLATE_BATCH_SEGMENTS = 200;

/** How long one batch may take at the model before the run gives up with `timeout`. */
export const TRANSLATE_BATCH_TIMEOUT_MS = 60_000;

/** HTTP 429: the per-minute limit, when the server gave no code of its own. */
export const TOO_MANY_REQUESTS = 429;

/** Pages one click keeps translating on its own, on the same site, before the person has to click again. */
export const TRANSLATE_KEEP_PAGES = 20;

/** Most characters in the first request: the top of the page comes back in seconds while the rest is still out. */
export const TRANSLATE_FIRST_BATCH_CHARS = 1_000;

/** Requests in flight at once for one page: a long page takes about a third of the time, and stays well within the server's burst. */
export const TRANSLATE_PARALLEL = 3;
