/**
 * The numbers the self-hosted license runs on.
 */

/** Cloud browsers a self-hosted server may run at once without a license key. */
export const SELF_HOST_FREE_CAP = 5;
/** Where the licensing module is vendored: server/vendor/oya-license.js. */
export const VENDOR_FILE = new URL('../../../vendor/oya-license.js', import.meta.url).href;
/** Where a self-hosted server sends its daily ping. */
export const PING_URL = 'https://oyabrowser.com/api/telemetry/ping';
/** Where to book a call with the founders: for more than self-serve sells, or anything plans do not cover. */
export const FOUNDERS_CALL = 'https://calendly.com/d/dvrm-r65-kkx/oya-founder-call';
/** Where a self-hoster buys a license, or more browsers: the hosted console, never this server's own. */
export const LICENSE_URL = 'https://oyabrowser.com/dashboard/billing#self-hosted';
/** The most browsers one self-serve license covers; above it, FOUNDERS_CALL. */
export const SELF_SERVE_MAX = 50;
