/**
 * REST routes for personas themselves: listing, creating, changing, cloning,
 * pinning to a proxy and deleting.
 *
 * A persona is one identity — fingerprint, cookie jar and proxy bound together
 * and stable. Rotation means choosing a different persona, never giving one a
 * new fingerprint.
 */
import type { Request, Response, Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import type { PersonaService } from './service.ts';
import { describeProfile, type Persona } from './model.ts';
import * as proxies from '../proxies/service.ts';
import { PREF_OPTIONS } from './fingerprint.ts';
import { announce, getKey, ownerScope } from '../../app/http.ts';
import { Status } from '../../platform/http-status.ts';
import { auditPersona, notFound, ownedPersona, refusedPrefs } from './route-support.ts';

/** Fields that are the device or the persona's identity, refused in an update. */
const FROZEN_FIELDS = ['seed', 'prefs', 'id', 'owner', 'isDefault', 'fingerprint'];

/** Listing, options, preview and creation. */
export function mountPersonaCatalog(router: Router, personas: PersonaService) {
  /** GET /personas — the caller's personas. */
  router.get('/personas', authMiddleware, listPersonas(personas));
  /** What the creation form may choose, so it never offers a timezone a platform cannot have. */
  router.get('/personas/options', authMiddleware, (req, res) => {
    res.json(PREF_OPTIONS);
  });
  /** The fingerprint these choices would produce. Persists nothing. */
  router.post('/personas/preview', authMiddleware, previewPersona(personas));
  /** POST /personas — a new persona with the requested device choices; invalid prefs are refused. */
  router.post('/personas', authMiddleware, createPersona(personas));
}

/** Changing, cloning, pinning, reading and deleting one persona. */
export function mountPersona(router: Router, personas: PersonaService) {
  /**
   * Name, cap and proxy hint only. The device itself — seed and prefs — is not
   * editable, and a body that tries is refused rather than silently trimmed.
   */
  router.put('/personas/:id', authMiddleware, updatePersona(personas));
  /** POST /personas/:id/clone — a new persona with the same device choices and a fresh seed. */
  router.post('/personas/:id/clone', authMiddleware, clonePersona(personas));
  /** Pin a persona to one proxy (`{proxyId}`), or unpin it (`{proxyId: null}`). */
  router.put('/personas/:id/proxy', authMiddleware, pinProxy(personas));
  /** GET /personas/:id — one persona. */
  router.get('/personas/:id', authMiddleware, getPersona(personas));
  /** DELETE /personas/:id — delete a persona and its logins, factors and credentials; refused for the default persona or one in use. */
  router.delete('/personas/:id', authMiddleware, deletePersona(personas));
}

/** Lists the caller's personas. */
const listPersonas = (personas: PersonaService) => (req: Request, res: Response) => {
  res.json({ personas: personas.list(getKey(req)).map((p) => personas.describe(p)) });
};

/** Describes the fingerprint the body's prefs would produce. */
const previewPersona = (personas: PersonaService) => (req: Request, res: Response) => {
  if (refusedPrefs(req, res)) return;
  res.json({ fingerprint: describeProfile(personas.preview(req.body?.prefs)) });
};

/** Creates a persona from the body. */
const createPersona = (personas: PersonaService) => (req: Request, res: Response) => {
  // Refuse rather than substitute: a persona is a device, and it must be the one asked for.
  if (refusedPrefs(req, res)) return;
  const { name, proxy, maxConcurrent, prefs } = req.body ?? {};
  const created = personas.create(getKey(req), { name, proxy, maxConcurrent, prefs });
  auditPersona(req, 'persona.create', created.id, { meta: { name: created.name, prefs: created.prefs } });
  announceCreated(req, created);
  res.status(Status.CREATED).json(personas.describe(created));
};

/** Tells the key's listeners a persona was created. */
const announceCreated = (req: Request, created: Persona, extra: Record<string, unknown> = {}) =>
  announce(getKey(req), 'persona.created', null, { personaId: created.id, name: created.name, ...extra });

/** Changes name, cap or proxy; a body touching the device is refused. */
const updatePersona = (personas: PersonaService) => (req: Request, res: Response) => {
  const body = req.body || {};
  if (refusedFrozen(body, res)) return;
  const { name, maxConcurrent, proxy } = body;
  const updated = personas.update(getKey(req), req.params.id as string, { name, maxConcurrent, proxy });
  if (!updated) return notFound(res);
  auditPersona(req, 'persona.update', updated.id, { meta: { fields: Object.keys(body) } });
  announce(getKey(req), 'persona.updated', null, { personaId: updated.id, fields: Object.keys(body) });
  res.json(personas.describe(updated));
};

/** Answers 400 when the body tries to change the device or identity; true if it did. */
function refusedFrozen(body: object, res: Response) {
  const frozen = FROZEN_FIELDS.filter((k) => k in body);
  if (frozen.length) res.status(Status.BAD_REQUEST).json({ error: frozenError(frozen) });
  return frozen.length > 0;
}

/** Why an update touching `frozen` fields is refused. */
const frozenError = (frozen: string[]) =>
  `${frozen.join(', ')} cannot change after creation — a persona's device is stable for its life. Clone it for a different device.`;

/** Copies a persona onto a fresh seed. */
const clonePersona = (personas: PersonaService) => (req: Request, res: Response) => {
  const clonedFrom = req.params.id as string;
  const created = personas.clone(getKey(req), clonedFrom, { name: req.body?.name });
  if (!created) return notFound(res);
  auditPersona(req, 'persona.create', created.id, { meta: { name: created.name, clonedFrom } });
  announceCreated(req, created, { clonedFrom });
  res.status(Status.CREATED).json(personas.describe(created));
};

/** Pins or unpins the persona's proxy. */
const pinProxy = (personas: PersonaService) => (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (!p) return;
  const proxyId = req.body?.proxyId ?? null;
  if (!pin(req, p.id, proxyId)) return res.status(Status.NOT_FOUND).json({ error: 'No such proxy' });
  auditPersona(req, 'persona.proxy', p.id, { meta: { proxyId } });
  res.json({ ok: true, proxy: proxies.assigned(p.id)?.toJSON() ?? null });
};

/** Unpins on a null id, else pins to that proxy; false when the caller has no such proxy. */
function pin(req: Request, personaId: string, proxyId: unknown) {
  if (proxyId !== null) return !!proxies.assign(ownerScope(req), personaId, String(proxyId));
  proxies.unassign(personaId);
  return true;
}

/** Describes one persona. */
const getPersona = (personas: PersonaService) => (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (p) res.json(personas.describe(p));
};

/** Deletes a persona with everything stored for it. */
const deletePersona = (personas: PersonaService) => (req: Request, res: Response) => {
  const id = req.params.id as string;
  if (!personas.remove(getKey(req), id)) return notFound(res);
  auditPersona(req, 'persona.delete', id);
  announce(getKey(req), 'persona.deleted', null, { personaId: id });
  res.json({ ok: true });
};
