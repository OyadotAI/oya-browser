/**
 * Messages about the connection itself: pings, live-view frames, command
 * results, relayed CDP and residential proxy metering.
 */
import { registry } from '../../registry.ts';
import { metrics } from '../../../../platform/metrics.ts';
import * as usage from '../../../../platform/usage.ts';
import { onBrowserMessage as onRelayMessage } from '../../cdp-relay.ts';
import { settleResult } from '../commands.ts';
import { MAX_PROXY_BYTES } from '../constants.ts';
import type { Handler } from './types.ts';

/** The browser pings; answer so it knows the server is there. */
export const ping: Handler = (conn) => {
  conn.heard();
  conn.send({ type: 'pong' });
};

/** The browser answered the server's ping. */
export const pong: Handler = (conn) => conn.heard();

/** A live-view frame, pushed to viewers and counted as egress. */
export const frame: Handler = ({ browserId, apiKey }, msg) => {
  if (!msg.data) return;
  registry.pushFrame(browserId, msg.data);
  metrics.frames.inc({ client: 'oya' });
  usage.record(apiKey, 'frames');
  usage.record(apiKey, 'bytes_out', msg.data.length);
};

/** The answer to a command the server sent; also tells us where the page is. */
export const cmdResult: Handler = (conn, msg) => {
  settleResult(conn.browserId, msg, conn.isCurrent());
  if (msg.data?.url) registry.updateUrl(conn.browserId, msg.data.url);
};

/** Residential proxy bytes counted in a sandbox we run (the vendor bills per GB), so the count is trusted. */
export const proxyBytes: Handler = (conn, msg) => {
  const bytes = Math.round(Number(msg.bytes));
  if (!conn.residentialProxy || !conn.isCurrent() || bytes <= 0) return;
  usage.record(conn.apiKey, 'residential_proxy_bytes', Math.min(bytes, MAX_PROXY_BYTES));
};

/** CDP relayed for a gateway client (cdp-relay.ts). */
export const cdpRelay: Handler = (conn, msg) => {
  if (conn.isCurrent()) onRelayMessage(conn.browserId, msg);
};
