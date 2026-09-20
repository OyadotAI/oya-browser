/**
 * Project membership: the signed-in user's own project list and invitations (mounted at /auth/projects),
 * plus the invite and remove actions the project's control routes call.
 */
import { Router } from 'express';
import { userAuthMiddleware } from '../auth/service.ts';
import { control, fault } from './service.ts';
import { Status } from '../../platform/http-status.ts';
import { AUTO_NAME, listProjects, joinProject, consoleAccess } from './http/projects.ts';
import { inviteMember, removeMember } from './http/members.ts';

export { AUTO_NAME, inviteMember, removeMember };

/** Routes for a signed-in user's projects, keyed by user rather than API key. */
export const projectAccountRouter = Router();
/** Every route here needs a signed-in user. */
projectAccountRouter.use(userAuthMiddleware);

/** GET /auth/projects — projects the user owns or has joined, with their role in each. */
projectAccountRouter.get('/', async (req, res) => res.json(await listProjects(req.user.id)));

/** POST /auth/projects/join — redeem a one-time invitation code and become a member with its role. */
projectAccountRouter.post('/join', async (req, res) => res.json(await joinProject(req.user.id, req.body?.code)));

/** POST /auth/projects/:id/access — a one-hour console credential for a project the user owns or belongs to. */
projectAccountRouter.post('/:id/access', async (req, res) => res.json(await consoleAccess(req.user.id, req.params.id)));

/** PATCH /auth/projects/:id — rename a project; owner only. */
projectAccountRouter.patch('/:id', async (req, res) => {
  res.json(await control().updateOwnedProject(req.user.id, req.params.id, { name: req.body?.name }));
});

/** DELETE /auth/projects/:id — delete a project the user owns; refused while browsers run unless stopBrowsers is true. */
projectAccountRouter.delete('/:id', async (req, res) => {
  res.json(
    await control().updateOwnedProject(req.user.id, req.params.id, {
      remove: true,
      stopBrowsers: req.body?.stopBrowsers === true,
    }),
  );
});

/** POST /auth/projects/:id/key — the project's API key; owner only, never cached. */
projectAccountRouter.post('/:id/key', async (req, res) => {
  const p = await control().store.get('project', req.params.id);
  if (!p || p.deletedAt || p.ownerUser !== req.user.id) throw fault('not_found', 'Project not found', Status.NOT_FOUND);
  res.set('Cache-Control', 'no-store');
  res.json({ key: control().projectKey(p) });
});

/** Errors become JSON with the fault's status and code (503 when it has none). */
projectAccountRouter.use((e, req, res, _next) =>
  res
    .status(e.status || Status.UNAVAILABLE)
    .json({ error: e.message, code: e.code, ...(e.active ? { active: e.active } : {}) }),
);
