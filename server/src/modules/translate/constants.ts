/**
 * Limits for page translation. The desktop browser batches a page's text to fit
 * these; its own copies are pinned to them by test, so a change here goes there too.
 */

/** Most characters of page text one translate request may carry: one model call's worth. */
export const TRANSLATE_BATCH_CHARS = 4_000;

/** Most text segments (DOM text nodes) one request may carry. */
export const TRANSLATE_BATCH_SEGMENTS = 200;

/** A BCP 47 language tag, loosely: the primary subtag and any subtags after it. */
export const TRANSLATE_TAG = /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/;
