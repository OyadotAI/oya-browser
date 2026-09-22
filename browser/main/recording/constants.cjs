/** Numbers the desktop recorder (main/recording/) runs on. */

/** Steps one recording may hold; past it the recording stops itself. */
const MAX_RECORDED_STEPS = 500;
/** How often the shell's step list is refreshed while recording. */
const RECORDING_REFRESH_MS = 400;
/** Random bytes in the recorder's per-page tag attribute. */
const ANALYZER_ATTR_BYTES = 4;
/** An upload this soon after a click in the same tab means that click opened the file picker. */
const FILE_PICKER_CLICK_MS = 60000;
/** How long the recorder waits for a page to answer one protocol command before giving up on it. */
const RECORDING_CDP_MS = 5000;
/** A page move is checked this long after it happens, so the step that caused it has arrived from the page. */
const PAGE_CHECK_SETTLE_MS = 500;
/** Clicks this soon before a double-click on the same element are that double-click's own two clicks. */
const DOUBLE_CLICK_MS = 1000;
/** The playbook format the server is sent. */
const PLAYBOOK_SCHEMA_VERSION = 2;

module.exports = {
  MAX_RECORDED_STEPS,
  FILE_PICKER_CLICK_MS,
  RECORDING_REFRESH_MS,
  ANALYZER_ATTR_BYTES,
  PLAYBOOK_SCHEMA_VERSION,
  RECORDING_CDP_MS,
  PAGE_CHECK_SETTLE_MS,
  DOUBLE_CLICK_MS,
};
