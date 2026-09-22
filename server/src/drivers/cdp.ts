/**
 * Outbound CDP driver.
 *
 * Drives any browser that exposes a Chrome DevTools Protocol endpoint: Anchor,
 * Browserbase, Steel, Hyperbrowser, or a plain Chrome started with
 * --remote-debugging-port. The control plane dials out, which is the opposite
 * direction to the Oya client that dials in.
 *
 * Capability parity with the Oya client comes from injecting the same
 * scripts/analyzer.js into the page, so analyze and click-by-element_id behave
 * identically rather than degrading to raw coordinates.
 *
 * This file is the facade; the work is split by concern under cdp/.
 */
export { CDPConnection, CdpConnectionError } from './cdp/connection.ts';
export { CDPDriver } from './cdp/driver.ts';
export { CDP_CAPABILITIES } from './cdp/actions.ts';
export { dial, opened, dialWithTimeout, endpointAt } from './cdp/dial.ts';
export { MAX_PAYLOAD_BYTES, UPSTREAM_CONNECT_MS } from './cdp/constants.ts';
