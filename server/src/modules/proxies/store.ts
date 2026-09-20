/**
 * The pool's in-memory state: registered proxies and which persona holds which.
 * Shared by the Proxy class (to count its personas) and the service.
 */

/** id -> proxy */
export const proxies = new Map();
/** personaId -> proxyId, so an identity keeps its exit. */
export const assignments = new Map();

/** The secrets scope a proxy's credentials are sealed under. */
export const scopeFor = (id) => `proxy:${id}`;
