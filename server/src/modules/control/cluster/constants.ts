/** Every number cross-replica routing runs on, by name. */

/** Largest WebSocket message bridged between replicas (256 MiB). */
export const MAX_BRIDGE_PAYLOAD = 268_435_456;
/** A signed hop older (or newer) than this is refused. */
export const HOP_MAX_AGE_MS = 30_000;
/** Longest wait for the owner's response headers; streams such as SSE may run longer. */
export const HEADER_TIMEOUT_MS = 180_000;
/** Longest wait for the owner to accept a bridged WebSocket. */
export const HANDSHAKE_TIMEOUT_MS = 10_000;
/** Close code for a bridged client whose upstream ended. */
export const GOING_AWAY = 1001;
/** The instance record is renewed once its lease has less than this left… */
export const INSTANCE_RENEW_BEFORE_MS = 20_000;
/** …and is extended to this far ahead. */
export const INSTANCE_LEASE_MS = 30_000;
/** Base URL for parsing a request path on its own. */
export const LOCAL_BASE = 'http://local';
/** The raw answer to an upgrade whose owner could not be reached. */
export const UNAVAILABLE_UPGRADE = 'HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\n\r\n';
