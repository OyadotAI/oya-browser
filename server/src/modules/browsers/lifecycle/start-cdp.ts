/**
 * Starting a CDP browser we dial out to: acquire a vendor session, take the
 * persona's slot, drive it as that persona, and register it.
 */
import { randomUUID } from 'crypto';
import { CDPDriver } from '../../../drivers/cdp.ts';
import { getAll as getAllCookies, getStorage, mergeStorage } from '../../personas/cookies.ts';
import * as keyConfig from '../../config/service.ts';
import { container } from '../../../app/container.ts';
import { browserCdpUrl } from './cdp-url.ts';
import { countBrowsers, forget } from './fleet.ts';
import { acquireSession, addCdpBrowser, claimSession, releaseQuietly } from './sessions.ts';
import { started, type Start } from './start-reply.ts';

/** Not yet layered: reads the persona service from the composition root. */
const { personas } = container;

/** Starts a CDP browser and answers with its gateway CDP URL. */
export async function launchCdp(start: Start) {
  const { browserId, session, driver } = await openCdp(start);
  const { req, key, persona } = start;
  const release = releaseBoth(session, persona, browserId);
  addCdpBrowser(req, key, browserId, session, { driver, persona, release });
  countBrowsers();
  started(start, await ready(start, browserId, session));
}

/** Everything up to a connected driver. */
async function openCdp({ req, key, wanted, persona }: Start) {
  const wsUrl = req.body?.wsUrl || keyConfig.envFor(key).OYA_CDP_WS_URL;
  const session = await acquireSession(req, key, wanted, wsUrl);
  const browserId = req.controlSession?.id || randomUUID();
  await claimSession(key, browserId, session);
  const driver = await driveAs(persona, browserId, session);
  return { browserId, session, driver };
}

/**
 * The concurrency slot is taken before the vendor session is driven, so a
 * capped persona does not leave a paid-for browser running with nothing
 * holding it.
 */
async function driveAs(persona, browserId, session) {
  await takeSlot(persona, browserId, session);
  return connectAs(persona, browserId, session);
}

/** Takes the persona's slot, or hands the session back and rethrows. */
async function takeSlot(persona, browserId, session) {
  try {
    personas.acquire(persona, browserId);
  } catch (capped) {
    await releaseQuietly(session);
    throw capped;
  }
}

/** Connects a driver as the persona, or frees the slot and session and rethrows. */
async function connectAs(persona, browserId, session) {
  try {
    return await new CDPDriver(driverOptions(persona, browserId, session)).connect();
  } catch (connectErr) {
    personas.release(persona, browserId);
    await releaseQuietly(session);
    throw connectErr;
  }
}

/** The driver wears the persona's fingerprint and logins, and saves storage back to it. */
function driverOptions(persona, browserId, session) {
  return {
    wsUrl: session.wsUrl,
    provider: session.provider,
    fingerprint: personas.fingerprintFor(persona),
    login: { cookies: getAllCookies(persona.id), origins: structuredClone(getStorage(persona.id)) },
    onStorage: (values) => mergeStorage(persona.id, values),
    onClose: () => forget(browserId),
  };
}

/** Hands the vendor session back, then frees the persona's slot. */
const releaseBoth = (session, persona, browserId) => async () => {
  await session.release();
  personas.release(persona, browserId);
};

/** The body for a browser ready to drive. */
async function ready({ req, persona }: Start, browserId, session) {
  return {
    id: browserId,
    provider: session.provider,
    persona: persona.id,
    status: 'ready',
    // Our gateway URL, not the vendor's: an agent handed this gets routing,
    // profiles and recording without knowing any of that exists.
    cdpUrl: await browserCdpUrl(req, browserId),
  };
}
