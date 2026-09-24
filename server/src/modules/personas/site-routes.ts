/**
 * REST routes for what a persona signs in with on its own: second factors and
 * site credentials. Secrets are write-only; nothing here ever reads one back.
 */
import type { Request, Response, Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import type { PersonaService } from './service.ts';
import * as mfa from '../challenges/mfa.ts';
import * as credentials from './credentials.ts';
import { Status } from '../../platform/http-status.ts';
import { auditPersona, ownedPersona } from './route-support.ts';

/** A persona's second factors. */
export function mountMfa(router: Router, personas: PersonaService) {
  /**
   * Configure a persona's second factor. The secret is write-only.
   *
   * An optional `domain` files the factor against one site, because a persona
   * that drives several portals meets several kinds of factor. Without it the
   * record is the persona-wide default, which is what every factor stored before
   * per-site keying still is.
   */
  router.put('/personas/:id/mfa', authMiddleware, setMfa(personas));
  /**
   * Which factors this persona holds: the persona-wide one, and the sites with one
   * of their own. Kinds only, never a secret, an operator setting a factor per
   * portal had no way to see which portals were done.
   */
  router.get('/personas/:id/mfa', authMiddleware, listMfa(personas));
  /** DELETE /personas/:id/mfa, clear the persona's second factor, for one site with ?domain= or the persona-wide default. */
  router.delete('/personas/:id/mfa', authMiddleware, clearMfa(personas));
}

/**
 * The cookie jar is still how a persona stays signed in. These exist for the
 * portals that expire a session server-side between runs and then demand a real
 * login, where an unattended run has nothing else to recover with.
 */
export function mountCredentials(router: Router, personas: PersonaService) {
  /** Which sites this persona can sign in to. Usernames only, never passwords. */
  router.get('/personas/:id/credentials', authMiddleware, listCredentials(personas));
  /** Store a site login. The password is write-only and never read back. */
  router.put('/personas/:id/credentials', authMiddleware, setCredentials(personas));
  /** DELETE /personas/:id/credentials?domain=, forget the persona's login for one site. */
  router.delete('/personas/:id/credentials', authMiddleware, clearCredentials(personas));
}

/** Stores a second factor, persona-wide or for one site. */
const setMfa = (personas: PersonaService) => async (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (!p) return;
  const { domain = null, ...config } = req.body || {};
  const site = domain ? credentials.domainOf(domain) : null;
  if (domain && !site) return res.status(Status.BAD_REQUEST).json({ error: 'domain is not a hostname' });
  const described: any = await mfa.set(p.id, config, site);
  auditPersona(req, 'mfa.configure', p.id, { meta: { type: described.type, domain: site } });
  res.json(described);
};

/** The persona-wide factor and the per-site ones, by kind. */
const listMfa = (personas: PersonaService) => (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (p) res.json({ ...mfa.describe(p.id), sites: mfa.list(p.id) });
};

/** Clears a second factor, for one site or persona-wide. */
const clearMfa = (personas: PersonaService) => async (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (!p) return;
  const site = req.query.domain ? credentials.domainOf(String(req.query.domain)) : null;
  await mfa.clear(p.id, site);
  auditPersona(req, 'mfa.clear', p.id, { meta: { domain: site } });
  res.json({ ok: true });
};

/** Lists the sites and usernames the persona has logins for. */
const listCredentials = (personas: PersonaService) => (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (p) res.json({ credentials: credentials.list(p.id) });
};

/** Seals a site login. */
const setCredentials = (personas: PersonaService) => async (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (!p) return;
  const { domain, username, password } = req.body || {};
  const described = await credentials.set(p.id, domain, { username, password });
  // The domain and username are the whole audit value here: knowing which
  // account was bound to which portal, and never the secret itself.
  auditPersona(req, 'credentials.configure', p.id, {
    meta: { domain: described.domain, username: described.username },
  });
  res.json(described);
};

/** Forgets the login for the `?domain=` site. */
const clearCredentials = (personas: PersonaService) => async (req: Request, res: Response) => {
  const p = ownedPersona(personas, req, res);
  if (!p) return;
  const site = credentials.domainOf(String(req.query.domain || ''));
  if (!site) return res.status(Status.BAD_REQUEST).json({ error: 'a domain query parameter is required' });
  const removed = await credentials.clear(p.id, site);
  auditPersona(req, 'credentials.clear', p.id, { meta: { domain: site }, outcome: removed ? 'ok' : 'error' });
  res.json({ ok: removed });
};
