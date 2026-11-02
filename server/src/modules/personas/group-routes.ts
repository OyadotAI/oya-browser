/**
 * REST routes: persona groups, the named sets of personas a browser start can
 * name instead of one persona. Each route calls the group service and answers;
 * the rules live in groups.ts and group-rules.ts. A group's name is what the
 * caller's code passes, so there is no rename: delete and make it again.
 */
import { Router, type Request, type Response } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { getKey } from '../../app/http.ts';
import { audit } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import type { GroupService } from './groups.ts';

/** Records a change to a group, by the calling key. */
const auditGroup = (req: Request, action: string, name: string) =>
  audit({ action: `persona_group.${action}`, actorKey: getKey(req), targetType: 'persona_group', targetId: name, req });

/** The group named in the path. */
const nameOf = (req: Request) => req.params.name as string;

/** Lists the caller's groups with their live members. */
const listGroups = (groups: GroupService) => async (req: Request, res: Response) => {
  res.json({ groups: await groups.list(getKey(req)) });
};

/** Makes a group: { name, personaIds }. */
const createGroup = (groups: GroupService) => async (req: Request, res: Response) => {
  const made = await groups.create(getKey(req), req.body);
  auditGroup(req, 'create', made.name);
  res.status(Status.CREATED).json(made);
};

/** Replaces a group's members: { personaIds }. */
const replaceGroup = (groups: GroupService) => async (req: Request, res: Response) => {
  const changed = await groups.replace(getKey(req), nameOf(req), req.body);
  auditGroup(req, 'update', changed.name);
  res.json(changed);
};

/** Deletes a group; its personas are not touched. */
const deleteGroup = (groups: GroupService) => async (req: Request, res: Response) => {
  await groups.remove(getKey(req), nameOf(req));
  auditGroup(req, 'delete', nameOf(req));
  res.json({ ok: true });
};

/** The persona group routes, over the service they are given. */
export function personaGroupRoutes(groups: GroupService) {
  const router = Router({ caseSensitive: true });
  /** GET /persona-groups, the caller's groups. */
  router.get('/persona-groups', authMiddleware, listGroups(groups));
  /** POST /persona-groups, a new group; 409 group_exists for a name the caller has. */
  router.post('/persona-groups', authMiddleware, createGroup(groups));
  /** PUT /persona-groups/:name, replace its members. */
  router.put('/persona-groups/:name', authMiddleware, replaceGroup(groups));
  /** DELETE /persona-groups/:name, delete it. */
  router.delete('/persona-groups/:name', authMiddleware, deleteGroup(groups));
  return router;
}
