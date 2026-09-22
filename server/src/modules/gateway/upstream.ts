/**
 * Dialling a browser's CDP WebSocket. The work moved to the CDP driver, which
 * owns the endpoint; the names stay importable from here.
 */
export { dial, opened, dialWithTimeout, endpointAt } from '../../drivers/cdp.ts';
