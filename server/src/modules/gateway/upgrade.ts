/**
 * Route an HTTP upgrade on /connect into a gateway session: authenticate,
 * forward to the replica that owns the target, rate-limit, then resume a
 * session, attach to a fleet browser, or start a new session.
 */
import { forwardGateway } from '../control/cluster.ts';
import { consume } from '../../platform/limits.ts';
import { Status } from '../../platform/http-status.ts';
import { authenticate } from './upgrade-auth.ts';
import { resume } from './upgrade-resume.ts';
import { attachToFleet } from './upgrade-attach.ts';
import { startSession } from './upgrade-start.ts';
import { denier, refuse, type Upgrade } from './upgrade-context.ts';

/** Route an HTTP upgrade on /connect into a gateway session. */
export async function handleUpgrade(req, socket, head) {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const deny = denier(socket);
  const caller = await authenticate(url, req, deny);
  if (!caller) return;
  const ctx: Upgrade = { req, socket, head, url, deny, ...caller };
  if ((await forwarded(ctx)) || !admitted(ctx)) return;
  return stageFor(url)(ctx);
}

/** A session or browser that lives on another replica is proxied there. */
async function forwarded(ctx: Upgrade) {
  const targetId = ctx.url.searchParams.get('browser') || ctx.url.searchParams.get('session');
  return !!targetId && (await forwardGateway(ctx.req, ctx.socket, ctx.head, ctx.authToken, ctx.token, targetId));
}

/** The per-key connect rate limit; refused with a 429. */
function admitted(ctx: Upgrade) {
  if (consume('connect', ctx.token).allowed) return true;
  refuse(ctx, 'rate_limited', Status.TOO_MANY_REQUESTS, 'Too Many Requests');
  return false;
}

/** ?session= resumes, ?browser= attaches, anything else starts a new session. */
function stageFor(url: URL) {
  if (url.searchParams.get('session')) return resume;
  if (url.searchParams.get('browser')) return attachToFleet;
  return startSession;
}
