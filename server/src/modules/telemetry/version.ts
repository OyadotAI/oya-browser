/**
 * The desktop app's version as a client reports it: in its auth message and
 * in a header on update checks. A client can send anything, so only a plain
 * release number is kept and everything else reads as unknown.
 */

/** What an unreported or malformed version reads as. */
export const UNKNOWN = 'unknown';
/** The header the app sends its version in when it checks for an update. */
export const VERSION_HEADER = 'x-oya-version';
/** A release number such as 1.0.115, optionally with a short pre-release tag. */
const RELEASE = /^\d{1,4}\.\d{1,4}\.\d{1,6}(-[0-9A-Za-z.]{1,20})?$/;

/** The reported version when it is a release number, else unknown. */
export const versionOf = (reported: unknown) => {
  const text = typeof reported === 'string' ? reported.trim() : '';
  return RELEASE.test(text) ? text : UNKNOWN;
};
