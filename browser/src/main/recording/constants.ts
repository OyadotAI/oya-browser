/** Numbers the desktop recorder (src/main/recording/) runs on. */

/** Steps one recording may hold; past it the recording stops itself. */
export const MAX_RECORDED_STEPS = 500;
/** How often the shell's step list is refreshed while recording. */
export const RECORDING_REFRESH_MS = 400;
/** Random bytes in the recorder's per-page tag attribute. */
export const ANALYZER_ATTR_BYTES = 4;
/** An upload this soon after a click in the same tab means that click opened the file picker. */
export const FILE_PICKER_CLICK_MS = 60000;
/** How long the recorder waits for a page to answer one protocol command before giving up on it. */
export const RECORDING_CDP_MS = 5000;
/** A page move is checked this long after it happens, so the step that caused it has arrived from the page. */
export const PAGE_CHECK_SETTLE_MS = 500;
/** Clicks this soon before a double-click on the same element are that double-click's own two clicks. */
export const DOUBLE_CLICK_MS = 1000;
/** The playbook format the server is sent. */
export const PLAYBOOK_SCHEMA_VERSION = 2;
/** Bound native document authorization and quiescence without a protocol fallback. */
export const NATIVE_RECORDING_SETTLE_MS = 5000;
/** Missing native capture is represented explicitly rather than claiming a complete demonstration. */
export const NATIVE_CAPTURE_ISSUE = 'Native recording could not capture a document. Review this part of the workflow.';
