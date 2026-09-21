/**
 * Resume an existing session: a client that dropped reconnects within the
 * grace period and gets the same browser back.
 */
import { metrics } from '../../platform/metrics.ts';
import { Status } from '../../platform/http-status.ts';
import { sessions } from './session-store.ts';
import { accept, auditAs, refuse, type Refusal, type Upgrade } from './upgrade-context.ts';

/** Why this caller may not resume the session, or null when it may. */
function resumeRefusal(existing, token): Refusal | null {
  if (!existing) return ['unknown_session', Status.NOT_FOUND, 'Not Found'];
  if (existing.apiKey !== token) return ['forbidden', Status.FORBIDDEN, 'Forbidden'];
  if (existing.client) return ['session_busy', Status.CONFLICT, 'Session In Use'];
  return null;
}

/** Reattaches the caller to the session named by ?session=. */
export function resume(ctx: Upgrade) {
  const existing = sessions.get(ctx.url.searchParams.get('session'));
  const refusal = resumeRefusal(existing, ctx.token);
  if (refusal) return refuse(ctx, ...refusal);
  accept(ctx, (client) => resumed(ctx, existing, client));
}

/** The client is back on its session. */
function resumed(ctx: Upgrade, existing, client) {
  existing.authToken = ctx.authToken;
  existing.attach(client);
  metrics.gatewayConnects.inc({ outcome: 'resumed' });
  auditAs(ctx, { action: 'gateway.session.resume', targetType: 'session', targetId: existing.id });
}
