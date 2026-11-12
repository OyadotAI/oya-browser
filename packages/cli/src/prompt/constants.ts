/**
 * The prompt layer's numbers: terminal geometry fallbacks and timings.
 */

/** Terminal width assumed when the stream does not report one. */
export const DEFAULT_COLUMNS = 80;
/** Terminal height assumed when the stream does not report one. */
export const DEFAULT_ROWS = 24;
/** Banner rules never grow wider than this. */
export const MAX_RULE_WIDTH = 74;
/** Spinner frame interval. */
export const SPINNER_FRAME_MS = 80;
/** Comfortable reading width for setup prose. */
export const SETUP_CONTENT_COLUMNS = 72;
/** Space reserved for the question rail. */
export const SETUP_INSET = 4;
/** A tab advances to the next terminal tab stop. */
export const TAB_COLUMNS = 8;
/** First printable ASCII code point. */
export const PRINTABLE_START = 32;
/** ASCII delete is a terminal control character. */
export const DELETE_CODE = 127;
/** East Asian glyphs and emoji generally occupy two terminal cells. */
export const WIDE_CELL_WIDTH = 2;
/** Menu labels sit two cells deeper than ordinary rail prose. */
export const CHOICE_INSET = 2;
