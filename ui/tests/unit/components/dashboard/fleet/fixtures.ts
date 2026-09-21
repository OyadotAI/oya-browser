/**
 * Browser rows for the fleet and panel tests.
 */
import type { BrowserRow } from '@/components/dashboard/types';
import type { FleetFilter } from '@/components/dashboard/fleet-strip';

/** A healthy desktop browser; override any field. */
export const row = (over: Partial<BrowserRow> = {}): BrowserRow => ({
  id: 'b-1',
  name: 'alpha',
  clientType: 'oya',
  provider: 'oya-desktop',
  persona: null,
  personaName: null,
  health: 'ok',
  connectedAt: new Date().toISOString(),
  lastSeen: new Date().toISOString(),
  currentUrl: 'https://example.com/',
  commands: 0,
  errors: 0,
  pending: 0,
  lastCommandAt: null,
  lastError: null,
  streaming: false,
  ...over,
});

/** No filter set. */
export const noFilter: FleetFilter = { health: null, provider: null, persona: null, text: '' };
