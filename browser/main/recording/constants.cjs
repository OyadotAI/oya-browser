/** Numbers the desktop recorder (main/recording/) runs on. */

/** Steps one recording may hold; past it the recording stops itself. */
const MAX_RECORDED_STEPS = 500;
/** How often the shell's step list is refreshed while recording. */
const RECORDING_REFRESH_MS = 400;
/** Random bytes in the recorder's per-page tag attribute. */
const ANALYZER_ATTR_BYTES = 4;
/** An upload this soon after a click in the same tab means that click opened the file picker. */
const FILE_PICKER_CLICK_MS = 60000;
/** The playbook format the server is sent. */
const PLAYBOOK_SCHEMA_VERSION = 2;

module.exports = {
  MAX_RECORDED_STEPS,
  FILE_PICKER_CLICK_MS,
  RECORDING_REFRESH_MS,
  ANALYZER_ATTR_BYTES,
  PLAYBOOK_SCHEMA_VERSION,
};
