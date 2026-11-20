/**
 * Every number and fixed string the MCP OAuth server runs on, by name.
 */

/** How long a person has to allow a connection on the console page. */
export const REQUEST_TTL_MS = 600_000;
/** How long an allowed code may wait to be exchanged; a client exchanges it at once. */
export const CODE_TTL_MS = 60_000;
/** Random bytes in a request id or an authorization code. */
export const SECRET_BYTES = 32;
/** The console page that asks the person to allow a connection. */
export const CONNECT_PAGE = '/connect';
/** The MCP endpoint the issued credential is for. */
export const RESOURCE_PATH = '/mcp/pool';
/** What the server calls itself in its resource metadata. */
export const RESOURCE_NAME = 'Oya Browser';
/** The credential's name, when the client registered without one. */
export const DEFAULT_CLIENT_NAME = 'MCP client';
/** A registered client that has not signed anyone in for this long is forgotten; the client registers again. */
export const CLIENT_TTL_MS = 7_776_000_000;
/** Longest client name kept; it becomes the credential's label. */
export const MAX_CLIENT_NAME = 80;
/** Most redirect URIs one client may register. */
export const MAX_REDIRECT_URIS = 5;
/** Registrations the whole server takes per hour: a backstop above the per-address limit that bounds how fast the client store can fill. */
export const REGISTRATIONS_PER_HOUR = 2_000;
/** The window that limit counts over. */
export const REGISTRATION_WINDOW_MS = 3_600_000;
/** What /authorize must carry before anything is redirected anywhere. */
export const AUTHORIZE_REQUIRED = { response_type: 'code', code_challenge_method: 'S256' };
/** The role a connector gets at most: driving browsers needs no more, and a leaked token then cannot change settings. */
export const CONNECTOR_MAX_ROLE = 'operator';
/** The error a refused or used-up code answers, as OAuth names it. */
export const INVALID_GRANT = 'Authorization code is invalid, expired or already used';
