/**
 * Lease renewal, on its own timer so a slow cleanup never lets a lease lapse:
 * advertise this replica, extend the sessions it holds, end expired human
 * control, and tell each browser its control mode.
 */
import { heartbeatInstance } from '../cluster.ts';
import { control, instanceId, terminal } from '../service.ts';
import { desktopState } from '../desktop.ts';
import { registry } from '../../browsers/registry.ts';
import { sessions as gateways } from '../../gateway/service.ts';
import { exclusive } from './state.ts';
import { SESSION_LEASE_MS, SESSION_RENEW_BEFORE_MS } from './constants.ts';

/** Advertises this replica, extends the leases on sessions it holds, ends expired human control, and tells each browser its control mode. */
export async function renewLeases() {
  await exclusive('heartbeating', async () => announce(await control().store.transact(renewOwned)));
}

/** The renewal transaction; returns each held session's control state for its browser. */
async function renewOwned(tx) {
  await heartbeatInstance(tx);
  const now = Date.now();
  // Only this replica's own attachments are read; the fleet is never scanned for renewal.
  const attached = await tx.getMany('session', [...registry.browsers.keys(), ...gateways.keys()]);
  const held = attached.filter((x) => x.instance === instanceId && !terminal.has(x.state));
  for (const x of held) renewOne(tx, x, now);
  return held.map((x) => [x.id, desktopState(x.id, x.control)]);
}

/** Extends the lease when it runs low and pauses a human takeover that has expired. */
function renewOne(tx, x, now) {
  if (x.leaseUntil < now + SESSION_RENEW_BEFORE_MS) x.leaseUntil = now + SESSION_LEASE_MS;
  if (x.control.mode === 'human' && x.control.expiresAt <= now) {
    x.control = { mode: 'paused', revision: Math.max(now, (x.control.revision || 0) + 1) };
    tx.emit(x.project, 'control.paused', x.id);
  }
}

/** Sends each browser whose socket is open its control mode. */
function announce(modes) {
  for (const [id, state] of modes) {
    const ws = registry.get(id)?.ws;
    if (ws?.readyState === 1) ws.send(JSON.stringify({ type: 'control_mode', mode: state.mode, state }));
  }
}
