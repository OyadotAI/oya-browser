/**
 * REST routes for the admin page: the overview, a person by email, and the
 * self-hosted licenses. Each is signed-in and admin only.
 */
import { Router } from 'express';
import { userAuthMiddleware } from '../auth/service.ts';
import { Status } from '../../platform/http-status.ts';
import { adminOnly } from './access.ts';
import * as admin from './service.ts';

/** The admin routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** Every admin route: signed in, and an admin. */
const guard = [userAuthMiddleware, adminOnly];

/** GET /admin/overview, accounts, plans, heaviest users, installs, downloads and the fleet. */
router.get('/admin/overview', ...guard, async (_req, res) => res.json(await admin.overview()));

/** GET /admin/users?email=, one person's plan, usage and keys. */
router.get('/admin/users', ...guard, async (req, res) => res.json(await admin.lookup(String(req.query.email || ''))));

/** GET /admin/licenses, every license issued. */
router.get('/admin/licenses', ...guard, async (_req, res) => res.json({ licenses: await admin.listLicenses() }));

/** POST /admin/licenses {licensee, maxConcurrent, expiresAt}, issues one; its key is in this answer only. */
router.post('/admin/licenses', ...guard, async (req, res) =>
  res.status(Status.CREATED).json(await admin.issue(req.body || {}, String(req.user.email))),
);

/** POST /admin/licenses/:id/revoke, revokes one. */
router.post('/admin/licenses/:id/revoke', ...guard, async (req, res) => res.json(await admin.revoke(req.params.id)));
