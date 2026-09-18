/**
 * REST routes: personas. The facade app/api.ts mounts; the routes themselves
 * live in persona-routes.ts (the personas) and site-routes.ts (what they sign
 * in with), registered here in the order Express must match them.
 */

import { Router } from 'express';
import type { PersonaService } from './service.ts';
import { mountPersonaCatalog, mountPersona } from './persona-routes.ts';
import { mountMfa, mountCredentials } from './site-routes.ts';

/** The persona REST routes, over the service they are given. */
export function personaRoutes(personas: PersonaService) {
  const router = Router({ caseSensitive: true });
  mountPersonaCatalog(router, personas);
  mountPersona(router, personas);
  mountMfa(router, personas);
  mountCredentials(router, personas);
  return router;
}
