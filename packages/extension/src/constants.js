/**
 * The extension's fixed values: where Oya lives, the HTTP statuses it answers
 * with, and the storage keys it keeps in this browser.
 */

/** The hosted Oya service every call goes to. The key is sent nowhere else. */
export const OYA_ORIGIN = 'https://oyabrowser.com';

/** The console page where a person creates keys and profiles. */
export const CONSOLE_URL = `${OYA_ORIGIN}/dashboard`;

/** The HTTP statuses the popup explains, by name. */
export const Status = Object.freeze({
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  PAYLOAD_TOO_LARGE: 413,
  GONE: 410,
  TOO_MANY_REQUESTS: 429,
});

/** A status no HTTP answer has: the request never reached Oya. */
export const NO_RESPONSE = 0;

/** The server's 403 text for a key it does not know (server/src/modules/auth/middleware.ts). */
export const UNKNOWN_KEY_ERROR = 'Invalid API key';

/** How the server's 400 for an import over its cookie cap begins (server/src/modules/browsers/http/pool.ts). */
export const IMPORT_CAP_ERROR = 'At most';

/** The role whose keys may read but never import (server ROLE_RULES). */
export const VIEWER_ROLE = 'viewer';

/** Where this browser keeps the API key and the last profile used. Local only, never synced. */
export const StorageKey = Object.freeze({ API_KEY: 'apiKey', LAST_PROFILE: 'lastProfile' });

/** Second-level labels that sit under a country code as a public suffix (co.uk, com.au). */
export const COUNTRY_SECOND_LEVELS = new Set(['co', 'com', 'net', 'org', 'gov', 'edu', 'ac', 'ne', 'or', 'go']);

/** How long a country-code top-level label is (uk, au, jp). */
export const COUNTRY_CODE_LENGTH = 2;

/** Labels in a registrable domain under a plain suffix (example.com). */
export const PLAIN_SITE_LABELS = 2;

/** Labels in a registrable domain under a country second level (example.co.uk). */
export const COUNTRY_SITE_LABELS = 3;
