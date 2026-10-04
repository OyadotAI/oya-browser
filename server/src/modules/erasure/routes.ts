/** REST routes: deleting the signed-in person's account (mounted at /auth/me). */
import { Router } from 'express';
import { userAuthMiddleware } from '../auth/service.ts';
import { audit } from '../../platform/audit.ts';
import type { Erasure } from './service.ts';

/** The account routes, over the erasure service they are given. */
export function accountRoutes(erasure: Erasure) {
  const router = Router({ caseSensitive: true });
  /** DELETE /auth/me, delete the account: its projects (erased after the grace period), keys, profile and sign-in. */
  router.delete('/', userAuthMiddleware, async (req: any, res) => {
    const result = await erasure.deleteAccount(req.user.id, req.impersonatedBy);
    audit({ action: 'account.delete', actorUser: req.user.id, targetType: 'account', targetId: req.user.id, req });
    res.json(result);
  });
  return router;
}
