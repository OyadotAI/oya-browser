/**
 * What a registered browser runs as: its persona's fingerprint with the proxy
 * that belongs to it, sent in `auth_ok` with the persona's cookie jar.
 */
import { desktopControl } from '../../control/desktop.ts';
import { getAll as getAllCookies, getStorage } from '../../personas/cookies.ts';
import { container } from '../../../app/container.ts';
import * as proxies from '../../proxies/service.ts';
import { fingerprint as personaOwner } from '../../../platform/audit.ts';
import { CLOUD_PROVIDER } from './constants.ts';

/** The fingerprint to run as, and whether it uses the metered residential gateway. */
export interface Identity {
  /** Fingerprint, with `proxy` filled in when the persona has one. */
  fingerprint: any;
  /** Whether the proxy is the operator's metered residential gateway. */
  residential: boolean;
}

/**
 * The persona's fingerprint with its proxy: the proxy is part of the identity.
 * The residential gateway is only for sandboxes we run, its credentials are the
 * operator's account, and a desktop could lift them.
 */
export function identityFor(apiKey: string, persona, provider: string): Identity {
  const fingerprint = container.personas.fingerprintFor(persona);
  const proxy = proxies.forPersona(personaOwner(apiKey), persona);
  const residential = provider === CLOUD_PROVIDER && !proxy && !fingerprint.proxy?.host && proxies.residential(persona);
  if (proxy || residential) attachProxy(fingerprint, persona, proxy, residential);
  return { fingerprint, residential: !!residential };
}

/** Puts the proxy on the fingerprint; metered, so the browser counts bytes (the vendor bills per GB). */
function attachProxy(fingerprint, persona, proxy, residential) {
  fingerprint.proxy = proxy ? proxies.credentials(proxy) : { ...residential, metered: true };
  const coherent = proxies.coherence(persona, fingerprint, proxy || residential);
  if (coherent.checked && 'ok' in coherent && !coherent.ok) console.warn(`[proxies] ${coherent.detail}`);
}

/** The `auth_ok` message: who the browser is and what it runs as. */
export async function welcomeMessage(apiKey: string, browserId: string, persona, fingerprint) {
  const control = await desktopControl(apiKey, browserId, 'get');
  return { type: 'auth_ok', browser_id: browserId, control, fingerprint, ...personaState(persona) };
}

/** The persona as the browser needs it: its name, cookie jar and localStorage, and this server's clock, which the cookie stamps are on. */
function personaState(persona) {
  const id = persona.id;
  return { persona: { id, name: persona.name }, cookies: getAllCookies(id), origins: getStorage(id), now: Date.now() };
}
