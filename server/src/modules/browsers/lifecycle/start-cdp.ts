/**
 * Starting a CDP browser we dial out to: acquire a vendor session, take the
 * persona's slot, drive it as that persona, and register it.
 */
import { randomUUID } from 'crypto';
import { registry } from '../registry.ts';
import { CDPDriver } from '../../../drivers/cdp.ts';
import { getAll as getAllCookies, getStorage, mergeStorage } from '../../personas/cookies.ts';
import * as keyConfig from '../../config/service.ts';
import { container } from '../../../app/container.ts';
import * as usage from '../../../platform/usage.ts';
import { browserCdpUrl } from './cdp-url.ts';
import { countBrowsers, forget } from './fleet.ts';
import { acquireSession, addCdpBrowser, claimSession, releaseQuietly } from './sessions.ts';
import { started, type Start } from './start-reply.ts';

/** Not yet layered: reads the persona service from the composition root. */
const { personas } = container;

/**
 * Starts a CDP browser and answers with its gateway CDP URL. Once the driver is
 * connected a session, a persona slot and a socket are all held, so a failure in
 * any later step lets go of all three: a slot that leaks leaves the persona
 * refusing every start, and it cannot be removed while it looks in use.
 */
export async function launchCdp(start: Start) {
  const opened = await openCdp(start);
  try {
    await register(start, opened);
  } catch (err) {
    await letGo(start, opened);
    throw err;
  }
}

/** Registers the connected browser and answers 201. */
async function register(start: Start, { browserId, session, driver }) {
  const { req, key, persona } = start;
  const release = releaseBoth(session, persona, browserId);
  addCdpBrowser(req, key, browserId, session, { engine: driver, persona, release });
  countBrowsers();
  started(start, await ready(start, browserId, session));
}

/**
 * Undoes a start that failed after connecting. A registered browser is removed,
 * which closes and releases it, and its usage clock is stopped: a browser the
 * caller was told failed must not keep billing browser-seconds. Otherwise the
 * driver is closed and the slot and session are let go of here.
 */
async function letGo({ key, persona }: Start, { browserId, session, driver }) {
  if (registry.isConnected(browserId)) return dropRegistered(key, browserId);
  driver.close();
  await releaseBoth(session, persona, browserId)().catch((err) => console.error('[start] release:', err.message));
}

/** Removes a browser that got as far as the registry, and ends its usage clock. */
function dropRegistered(key, browserId) {
  registry.remove(browserId);
  usage.browserDisconnected(key, browserId);
  countBrowsers();
}

/** Everything up to a connected driver. */
async function openCdp({ req, key, wanted, persona }: Start) {
  const wsUrl = req.body?.wsUrl || keyConfig.envFor(key).OYA_CDP_WS_URL;
  const browserId = req.controlSession?.id || randomUUID();
  const session = await acquireSession(req, key, wanted, wsUrl, browserId);
  await claimOrRelease(key, browserId, session);
  const driver = await driveAs(persona, browserId, session);
  return { browserId, session, driver };
}

/** Claims the session, or hands it back and rethrows: a refusal here would otherwise leave it, and its endpoint, held. */
async function claimOrRelease(key, browserId, session) {
  try {
    await claimSession(key, browserId, session);
  } catch (refused) {
    await releaseQuietly(session);
    throw refused;
  }
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

/**
 * Frees the persona's slot, then hands the vendor session back. The slot goes
 * first: freeing it cannot fail, while a vendor may refuse or time out, and a
 * slot held behind a vendor's 500 leaves the persona refusing every start.
 */
const releaseBoth = (session, persona, browserId) => async () => {
  personas.release(persona, browserId);
  await session.release();
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
