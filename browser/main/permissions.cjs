/**
 * What a page may do without asking. Electron grants every permission unasked:
 * any page could open the camera and microphone, read the location, and list
 * the machine's real devices by name, and `navigator.permissions` said
 * "granted" to everything, which no browser does and detectors read in one call.
 *
 * There is no permission prompt in this browser, so what Chrome would ask about
 * is refused, and what Chrome allows unasked is allowed. The page injection
 * reports a refused permission as Chrome's "prompt" (anonymity/stealth.js).
 * ponytail: no prompt, so a site that needs the camera cannot have it; add a prompt in the shell when a customer needs one.
 */

/** Permissions Chrome grants without asking, in Electron's names. */
const UNASKED = [
  'fullscreen',
  'pointerLock',
  'keyboardLock',
  'clipboard-sanitized-write',
  'background-sync',
  'sensors',
];

/** Whether `permission` is one Chrome grants without asking. */
const unasked = (permission) => UNASKED.includes(permission);

/** Refuses what Chrome would prompt for, on requests and on checks alike. Governance, where configured, then refuses everything. */
function installPermissions(ses) {
  ses.setPermissionRequestHandler((_contents, permission, callback) => callback(unasked(permission)));
  ses.setPermissionCheckHandler((_contents, permission) => unasked(permission));
}

module.exports = { installPermissions, unasked };
