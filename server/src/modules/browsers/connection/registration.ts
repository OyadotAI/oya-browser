/**
 * Recording an admitted browser: the control plane adopts the session, the
 * registry lists it, and usage starts counting.
 */
import { control, hash } from '../../control/service.ts';
import { registry } from '../registry.ts';
import { metrics } from '../../../platform/metrics.ts';
import * as usage from '../../../platform/usage.ts';
import { track } from '../../telemetry/index.ts';
import { isInternal } from '../../../drivers/vocabulary.ts';
import * as keyConfig from '../../config/service.ts';
import { isProvisioned } from '../../../drivers/sandbox.ts';
import {
  ACTION_NAME,
  CLAIMABLE_PROVIDERS,
  CLOUD_PROVIDER,
  DEFAULT_PROVIDER,
  MAX_ANNOUNCED_ACTIONS,
  MAX_BROWSERS_PER_KEY,
} from './constants.ts';

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

/** The platforms the desktop app reports; anything else is text a client made up, and reads as unknown. */
const PLATFORMS = new Set(['MacIntel', 'Win32', 'Linux x86_64', 'Linux aarch64']);

/** A desktop's platform as the event names it: one of the known values, else unknown. */
const platformOf = (reported: unknown) => (PLATFORMS.has(String(reported)) ? String(reported) : 'unknown');

/** Remembers that this key has a desktop, and counts the first time as the one worth telling the owner about. */
async function noteDesktop(reg: Registration, msg) {
  const first = !keyConfig.get(reg.apiKey).desktop_seen_at;
  await keyConfig.set(reg.apiKey, { desktop_seen_at: new Date().toISOString() });
  track.desktopConnected(reg.apiKey, { platform: platformOf(msg.host_platform), first });
}

/** Lists the browser and starts counting it. */
export async function list(reg: Registration, provider: string, msg) {
  addToRegistry(reg, provider, msg);
  if (provider === DEFAULT_PROVIDER) await noteDesktop(reg, msg);
  metrics.wsConnections.inc({ outcome: 'ok' });
  metrics.browsersConnected.set({}, registry.browsers.size);
  usage.browserConnected(reg.apiKey, reg.browserId);
}

/**
 * The actions a browser said it does, or null to use the Oya list: an older
 * app sends none, and a list that is not an array of plain action names, or
 * is too long, is a client misbehaving. Its names are only ever compared,
 * never used as keys, so "__proto__" is as harmless here as any other name.
 */
function announcedActions(announced: unknown): string[] | null {
  if (announced === undefined) return null;
  const usable = Array.isArray(announced) && announced.length <= MAX_ANNOUNCED_ACTIONS;
  // Internal names are dropped: only the server sends them, and the detail must not advertise what callers are refused.
  if (usable && announced.every((a) => typeof a === 'string' && ACTION_NAME.test(a)))
    return announced.filter((a) => !isInternal(a)).sort();
  console.warn('[connection] a browser announced an unusable action list; using the Oya list');
  return null;
}

/** The registry entry: what the dashboard and the command path see of the browser. */
function addToRegistry({ apiKey, browserId, persona, ws, authToken }: Registration, provider: string, msg) {
  const name = msg.browser_name || 'Browser';
  const actions = announcedActions(msg.actions);
  registry.add(browserId, { ws, apiKey, name, clientType: 'oya', persona, provider, cdp: msg.cdp === true, actions });
  registry.get(browserId).authToken = authToken;
}
