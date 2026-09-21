/**
 * Files the person asked to save (diagnostics, Playwright exports): written
 * owner-only, since both can carry what a page showed.
 */
const fs = require('fs');
const { PRIVATE_FILE_MODE } = require('../app/constants.cjs');

/** Writes `text` to `file`, owner-only, synchronously. */
function writePrivateFileSync(file, text) {
  fs.writeFileSync(file, text, { mode: PRIVATE_FILE_MODE });
}

/** Writes `text` to `file`, owner-only. */
function writePrivateFile(file, text) {
  return fs.promises.writeFile(file, text, { mode: PRIVATE_FILE_MODE });
}

module.exports = { writePrivateFileSync, writePrivateFile };
