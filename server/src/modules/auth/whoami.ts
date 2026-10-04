/**
 * GET /auth/whoami: the account and project the calling credential acts for,
 * so the desktop app, signed in with a key rather than a session, can say who
 * it is signed in as. Only the caller's own account is ever named: a stored
 * key answers for its owner, a member's credential for that member, and any
 * other credential (an environment key, a fleet token) for no one. A share
 * link never gets here: authMiddleware confines it to its own browser.
 */
import { Router } from 'express';
import { authMiddleware } from './middleware.ts';
import { getKeyOwner } from './api-keys.ts';
import { findProfile } from './repository.ts';
import { control } from '../control/service.ts';

/** The caller's plan by user id, or null on a server without plans. */
export type PlanOf = (userId: string) => Promise<string | null>;

/** The user a principal acts for: a member credential's member, a key's owner, or null. */
async function accountOf(principal): Promise<string | null> {
  if (principal.credentialId) return principal.memberUser || null;
  return getKeyOwner(principal.key);
}

/** The answer for one principal: its account (when it has one), its project and its role. */
async function whoami(principal, planOf: PlanOf) {
  const [userId, project] = await Promise.all([accountOf(principal), control().project(principal.key)]);
  const profile = userId ? await findProfile(userId) : null;
  const account = { email: profile?.email ?? null, name: profile?.display_name ?? null };
  const plan = profile ? await planOf(userId) : null;
  return { ...account, plan, role: principal.role, project: { id: project.id, name: project.name } };
}

/** The whoami route, with billing's plan lookup passed in by the composition root. */
export function whoamiRoutes(planOf: PlanOf) {
  const router = Router({ caseSensitive: true });
  /** GET /auth/whoami, read-only: `{ email, name, plan, role, project: { id, name } }` for the caller. */
  router.get('/auth/whoami', authMiddleware, async (req, res) => res.json(await whoami(req.principal, planOf)));
  return router;
}
