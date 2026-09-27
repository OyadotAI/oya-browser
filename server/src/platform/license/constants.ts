/**
 * The numbers the self-hosted license runs on.
 */

/** Cloud browsers a self-hosted server may run at once without a license key. */
export const SELF_HOST_FREE_CAP = 5;
/** Where the licensing module is vendored: server/vendor/oya-license.js. */
export const VENDOR_FILE = new URL('../../../vendor/oya-license.js', import.meta.url).href;
/** Where a self-hosted server sends its daily ping. */
export const PING_URL = 'https://oyabrowser.com/api/telemetry/ping';
/** Who to write to for a license. */
export const LICENSE_EMAIL = 'sales@getoya.ai';
