/**
 * A browser as the API lists it: identity, health and counters, without
 * activity.
 */
import { healthOf } from './health.ts';

/** Who and what the browser is. */
const identity = (id, b) => ({
  id,
  name: b.name,
  clientType: b.clientType,
  provider: b.provider,
  persona: b.persona?.id || null,
  personaName: b.persona?.name || null,
  health: healthOf(b),
});

/** When it connected and what it has done since. */
const progress = (b) => ({
  connectedAt: b.connectedAt.toISOString(),
  lastSeen: b.lastSeen.toISOString(),
  currentUrl: b.currentUrl,
  commands: b.commands,
  errors: b.errors,
  pending: b.pending,
});

/** Its latest command and whether anyone is watching it. */
const latest = (b) => ({
  lastCommandAt: b.lastCommandAt ? b.lastCommandAt.toISOString() : null,
  lastError: b.lastError,
  streaming: b.streamViewers.size > 0,
});

/** The API row for browser `id` with record `b`. */
export const rowOf = (id, b) => ({ ...identity(id, b), ...progress(b), ...latest(b) });
