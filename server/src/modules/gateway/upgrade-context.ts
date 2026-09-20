/**
 * What every stage of a /connect upgrade shares: the request, the caller, and
 * how to refuse or accept it.
 */
import { metrics } from '../../platform/metrics.ts';
import { audit } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { wss } from './session-store.ts';

/** One /connect upgrade in progress. */
export interface Upgrade {
  /** The HTTP upgrade request. */
  req: any;
  /** Its raw socket. */
  socket: any;
  /** The first packet of the upgraded stream. */
  head: any;
  /** The request URL, parsed. */
  url: URL;
  /** Answers the raw socket with an HTTP error and closes it. */
  deny: (code: number, message: string) => void;
  /** The caller's API key. */
  token: string;
  /** The credential the caller presented (a redeemed ticket's token, or the key). */
  authToken: string;
}

/** A refusal: [metrics outcome, status, message]. */
export type Refusal = [string, number, string];

/** A deny() for the raw socket: writes an HTTP error and closes. */
export const denier = (socket) => (code, message) => {
  socket.write(`HTTP/1.1 ${code} ${message}\r\nConnection: close\r\n\r\n`);
  socket.destroy();
};

/** Counts the refusal and answers it. */
export function refuse(ctx: Upgrade, outcome: string, status: number, message: string) {
  metrics.gatewayConnects.inc({ outcome });
  ctx.deny(status, message);
}

/** Completes the WebSocket upgrade and hands the client over. */
export function accept(ctx: Upgrade, onClient: (client) => void) {
  wss.handleUpgrade(ctx.req, ctx.socket, ctx.head, onClient);
}

/** Audits an event of this upgrade, as the caller. */
export function auditAs(ctx: Upgrade, event: Record<string, any>) {
  audit({ actorKey: ctx.token, req: ctx.req, ...event } as any);
}

/** Answers 503 when the cause was unavailability, else 502. */
export function denyUpstream(ctx: Upgrade, status) {
  if (status === Status.UNAVAILABLE) ctx.deny(Status.UNAVAILABLE, 'Service Unavailable');
  else ctx.deny(Status.BAD_GATEWAY, 'Bad Gateway');
}
