/**
 * REST routes for the admin page: the overview, a person by email, and the
 * self-hosted licenses. Each is signed-in and admin only.
 */
import { Router } from 'express';
import { userAuthMiddleware } from '../auth/service.ts';
import { Status } from '../../platform/http-status.ts';
import { audit } from '../../platform/audit.ts';
import { adminOnly } from './access.ts';
import * as admin from './service.ts';
import { setPlan, grant } from './billing.ts';

/** The admin routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** Every admin route: signed in, and an admin. */
const guard = [userAuthMiddleware, adminOnly];

/** GET /admin/overview, accounts, plans, heaviest users, installs, downloads and the fleet. */
router.get('/admin/overview', ...guard, async (_req, res) => res.json(await admin.overview()));

/** Audits an admin's action on a person or a license, under the admin. */
function auditAdmin(req, action: string, targetType: string, targetId: string) {
  audit({ action, actorKey: null, actorUser: req.user.id, targetType, targetId, req });
}

/** GET /admin/users?email=, one person's plan, usage and keys. Every lookup is audited, found or not. */
router.get('/admin/users', ...guard, async (req, res) => {
  const email = String(req.query.email || '');
  auditAdmin(req, 'admin.user.lookup', 'account', email.trim().toLowerCase());
  res.json(await admin.lookup(email));
});

/** POST /admin/users/:id/impersonate, a one-hour "Login as" token for that customer. */
router.post('/admin/users/:id/impersonate', ...guard, async (req, res) =>
  res.json(await admin.impersonate(req.params.id, req.user, req)),
);

/** GET /admin/licenses, every license issued. */
router.get('/admin/licenses', ...guard, async (_req, res) => res.json({ licenses: await admin.listLicenses() }));

/** POST /admin/licenses {licensee, maxConcurrent, expiresAt}, issues one; its key is in this answer only. */
router.post('/admin/licenses', ...guard, async (req, res) => {
  const issued = await admin.issue(req.body || {}, String(req.user.email));
  auditAdmin(req, 'admin.license.issue', 'license', issued.id);
  res.status(Status.CREATED).json(issued);
});

/** POST /admin/licenses/:id/revoke, revokes one. */
router.post('/admin/licenses/:id/revoke', ...guard, async (req, res) => {
  const revoked = await admin.revoke(req.params.id);
  auditAdmin(req, 'admin.license.revoke', 'license', req.params.id);
  res.json(revoked);
});

/** Records the administrator, target, reason and exact adjustment. */
function auditBilling(req, action: string, meta: object) {
  audit({ action, actorUser: req.user.id, targetType: 'account', targetId: req.params.id, meta, req });
}

/** PUT /admin/users/:id/plan, changes only the customer's access override. */
router.put('/admin/users/:id/plan', ...guard, async (req, res) => {
  const result = await setPlan(req.params.id, req.body || {}, req.user.id);
  auditBilling(req, 'admin.billing.plan', result);
  res.json(result);
});

/** POST /admin/users/:id/grants, adds retry-safe hours or hosted model dollars this period. */
router.post('/admin/users/:id/grants', ...guard, async (req, res) => {
  const result = await grant(req.params.id, req.body || {}, req.user.id);
  auditBilling(req, 'admin.billing.grant', result);
  res.json(result);
});
