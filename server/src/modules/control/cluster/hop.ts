/**
 * Signed hops between replicas: one replica proves a forwarded request came
 * from a peer with an HMAC under the shared cluster secret.
 */
import { createHmac, timingSafeEqual } from 'node:crypto';
import { fault } from '../service.ts';
import { Status } from '../../../platform/http-status.ts';
import { HOP_MAX_AGE_MS, LOCAL_BASE } from './constants.ts';

/** HMAC over timestamp, method and path with the shared cluster secret; one replica proves a hop to another with it. */
function signature(timestamp, method, path) {
  if (!process.env.OYA_CLUSTER_SECRET)
    throw fault('cluster_unconfigured', 'Cross-replica routing requires OYA_CLUSTER_SECRET', Status.UNAVAILABLE);
  return createHmac('sha256', process.env.OYA_CLUSTER_SECRET).update(`${timestamp}:${method}:${path}`).digest('hex');
}

/** The X-Oya-Hop value for a request to `path`, signed now. */
export function hopHeader(method, path) {
  const timestamp = String(Date.now());
  return `${timestamp}.${signature(timestamp, method, path)}`;
}

/** Throws unless a request carrying X-Oya-Hop is signed by a peer within the last 30 seconds; requests without it pass. */
export function verifyHop(req) {
  const hop = req.headers['x-oya-hop'];
  if (!hop) return;
  const [timestamp, supplied] = String(hop).split('.');
  if (!/^\d+$/.test(timestamp) || Math.abs(Date.now() - Number(timestamp)) > HOP_MAX_AGE_MS)
    throw fault('invalid_hop', 'Expired cluster request', Status.FORBIDDEN);
  if (!matches(supplied, signature(timestamp, req.method, req.originalUrl || req.url)))
    throw fault('invalid_hop', 'Invalid cluster request', Status.FORBIDDEN);
}

/** An already-forwarded request is never forwarded again: ownership moved while it was in flight. */
export function refuseRehop(req, message) {
  if (req.headers['x-oya-hop']) throw fault('owner_changed', message, Status.UNAVAILABLE);
}

/** Constant-time comparison of the supplied and expected signatures. */
function matches(supplied, expected) {
  const a = Buffer.from(supplied || ''),
    b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}

/** The owner's URL for the same path and query, minus the credentials named in `strip`. */
export function retarget(ownerUrl, path, strip) {
  const target = new URL(ownerUrl);
  const requested = new URL(path, LOCAL_BASE);
  target.pathname = requested.pathname;
  target.search = requested.search;
  for (const name of strip) target.searchParams.delete(name);
  return target;
}
