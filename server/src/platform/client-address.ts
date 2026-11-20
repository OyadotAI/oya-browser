/**
 * The caller's address, read the same way everywhere: rate limits key on it and
 * the audit log records it. Hops a client writes into X-Forwarded-For come first
 * and are worth nothing; our own proxies append theirs after, so the caller is
 * the hop the outermost trusted proxy appended, counted from the right.
 */
import { isIP } from 'node:net';
import { clientIpHeader, trustedProxyHops } from './constants.ts';

/** A request as far as its address goes. */
type AddressedRequest = {
  /** Read for x-forwarded-for. */
  headers?: Record<string, unknown>;
  /** The TCP peer, used when no trusted proxy forwarded the request. */
  socket?: {
    /** The peer's address. */
    remoteAddress?: string;
  };
};

/** The X-Forwarded-For hops, trimmed, empty ones dropped. */
const forwardedHops = (req: AddressedRequest) =>
  String(req.headers?.['x-forwarded-for'] || '')
    .split(',')
    .map((hop) => hop.trim())
    .filter(Boolean);

/**
 * The caller's address, or null when nothing names one. With fewer hops than
 * trusted proxies the request did not come through all of them, so the last hop
 * (the nearest proxy's view) is the most that can be believed.
 */
export function forwardedClient(req: AddressedRequest): string | null {
  return proxySetAddress(req) || forwardedHop(req);
}

/** The address our proxy wrote into OYA_CLIENT_IP_HEADER, when one is configured and it holds an IP. */
function proxySetAddress(req: AddressedRequest): string | null {
  const header = clientIpHeader();
  const value = header ? String(req.headers?.[header] ?? '').trim() : '';
  return value && isIP(value) ? value : null;
}

/** The X-Forwarded-For hop the outermost trusted proxy appended, else the socket. */
function forwardedHop(req: AddressedRequest): string | null {
  const hops = forwardedHops(req);
  const trusted = trustedProxyHops();
  const hop = hops.length >= trusted ? hops.at(-trusted) : hops.at(-1);
  // A hop a client could write is only worth keeping if it is an address at all.
  return (hop && isIP(hop) ? hop : null) || req.socket?.remoteAddress || null;
}

/** The caller's address for keying a rate limit; 'unknown' when nothing names one. */
export const clientAddress = (req: AddressedRequest) => forwardedClient(req) || 'unknown';
