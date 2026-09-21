/**
 * Every number key settings run on, by name: how secrets are masked and how the
 * fallback file is written.
 */

/** Trailing characters of a secret shown in its masked form. */
export const MASK_TAIL = 4;
/** Indent of the fallback settings file. */
export const FILE_INDENT = 2;
/** The fallback file holds sealed credentials: owner read and write only. */
export const FILE_MODE = 0o600;
