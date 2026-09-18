/** Route attachments to their leased owner. Forwarded requests keep the original credential and scope. */
import { instanceId } from './service.ts';
import { INSTANCE_LEASE_MS, INSTANCE_RENEW_BEFORE_MS } from './cluster/constants.ts';

export { ownerFor } from './cluster/owner.ts';
export { forwardHttp } from './cluster/forward-http.ts';
export { forwardGateway } from './cluster/forward-gateway.ts';

/** This replica's routable origin, or null when it runs alone. */
export function clusterOrigin() {
  if (!process.env.OYA_INSTANCE_URL) return null;
  const url = new URL(process.env.OYA_INSTANCE_URL);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || !process.env.OYA_CLUSTER_SECRET)
    throw new Error('Instance routing requires an HTTP(S) URL and cluster secret');
  return url.origin;
}

/** Advertise this replica's origin. Renews well before expiry so idle heartbeats write nothing; maintenance forgets processes gone a day. */
export async function heartbeatInstance(tx) {
  const origin = clusterOrigin();
  if (!origin) return;
  const now = Date.now(),
    current = await tx.get('instance', instanceId);
  if (current?.url !== origin || current.leaseUntil < now + INSTANCE_RENEW_BEFORE_MS)
    tx.put('instance', instanceId, { id: instanceId, url: origin, leaseUntil: now + INSTANCE_LEASE_MS });
}
