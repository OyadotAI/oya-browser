/**
 * Recording an admitted browser: the control plane adopts the session, the
 * registry lists it, and usage starts counting.
 */
import { control, hash } from '../../control/service.ts';
import { registry } from '../registry.ts';
import { metrics } from '../../../platform/metrics.ts';
import * as usage from '../../../platform/usage.ts';
import * as keyConfig from '../../config/service.ts';
import { isProvisioned } from '../../../drivers/sandbox.ts';
import { CLAIMABLE_PROVIDERS, CLOUD_PROVIDER, DEFAULT_PROVIDER, MAX_BROWSERS_PER_KEY } from './constants.ts';

/** The persona fields registration uses. */
export interface PersonaSlot {
  /** Persona id. */
  id: string;
  /** Its concurrency cap, enforced by the control plane too. */
  maxConcurrent: number;
}

/** What registration needs to know about the browser. */
export interface Registration {
  /** Key it registers under. */
  apiKey: string;
  /** Its id. */
  browserId: string;
  /** The persona it runs as. */
  persona: PersonaSlot;
  /** Its socket. */
  ws: any;
  /** The credential as presented, kept for live-view re-checks. */
  authToken: string;
}

/**
 * A cloud sandbox says so at enrolment; the dashboard's Stop needs to know,
 * because stopping a cloud browser means destroying its sandbox.
 */
export function providerOf(browserId: string, claimed?: string) {
  if (isProvisioned(browserId)) return CLOUD_PROVIDER;
  return CLAIMABLE_PROVIDERS.includes(claimed) ? claimed : DEFAULT_PROVIDER;
}

/** A managed browser must present the enrolment token its session was created with. */
async function checkEnrolment(browserId: string, token?: string) {
  const durable = await control().store.get('session', browserId);
  if (durable?.enrollmentHash && durable.enrollmentHash !== hash(String(token || ''))) {
    throw new Error('Managed browser enrollment token required');
  }
}

/** The control plane adopts the session under the key and persona caps. */
export async function adopt(reg: Registration, provider: string, enrollmentToken?: string) {
  await checkEnrolment(reg.browserId, enrollmentToken);
  await control().adopt(reg.apiKey, {
    id: reg.browserId,
    provider,
    persona: reg.persona.id,
    personaLimit: reg.persona.maxConcurrent,
    maxConcurrent: MAX_BROWSERS_PER_KEY,
  });
}

/** Lists the browser and starts counting it. */
export async function list(reg: Registration, provider: string, msg) {
  addToRegistry(reg, provider, msg);
  if (provider === DEFAULT_PROVIDER) await keyConfig.set(reg.apiKey, { desktop_seen_at: new Date().toISOString() });
  metrics.wsConnections.inc({ outcome: 'ok' });
  metrics.browsersConnected.set({}, registry.browsers.size);
  usage.browserConnected(reg.apiKey, reg.browserId);
}

/** The registry entry: what the dashboard and the command path see of the browser. */
function addToRegistry({ apiKey, browserId, persona, ws, authToken }: Registration, provider: string, msg) {
  const name = msg.browser_name || 'Browser';
  registry.add(browserId, { ws, apiKey, name, clientType: 'oya', persona, provider, cdp: msg.cdp === true });
  registry.get(browserId).authToken = authToken;
}
