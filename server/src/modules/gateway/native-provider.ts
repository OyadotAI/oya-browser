/** Provision one cold native Oya worker; external protocol frames use its browser-owned adapter only. */
import { createSandbox, removeSandbox, isConfigured } from '../../drivers/sandbox.ts';
import { registry } from '../browsers/registry.ts';
import { container } from '../../app/container.ts';
import { control, projectId } from '../control/service.ts';
import { prepareNativeProfile, saveNativeProfile } from './native-profile.ts';
import { NATIVE_ENROLL_TIMEOUT_MS, NATIVE_ENROLL_POLL_MS } from './constants.ts';

/** Cold hydration precedes allocation; failed provisioning always attempts to retire the worker. */
export async function acquireNative(start) {
  if (!isConfigured(start.token)) throw Error('Native cloud runtime is not configured');
  const persona = await prepareNativeProfile(start);
  try {
    return await connect(start, persona, await launch(start, persona));
  } catch (error) {
    await retire(start, persona);
    throw error;
  }
}

/** The welcome hydrates cookies and storage; a correlated capture confirms that setup has completed. */
async function launch(start, persona) {
  const spec = { apiKey: start.token, browserId: start.reservation.id, persona: persona.id, name: start.profileName };
  await createSandbox(spec);
  const browser = await enrolled(start, persona);
  await browser.driver.cookies();
  return browser;
}

/** Wait for this reserved browser, not whichever browser most recently joined the fleet. */
async function enrolled(start, persona) {
  const deadline = Date.now() + NATIVE_ENROLL_TIMEOUT_MS;
  while (Date.now() < deadline) {
    await control().assertProvisioning(start.token, start.reservation.id);
    const browser = registry.get(start.reservation.id);
    if (browser) return checked(start, persona, browser);
    await new Promise((resolve) => setTimeout(resolve, NATIVE_ENROLL_POLL_MS));
  }
  throw Error('Native gateway enrollment timed out');
}

/** Do not accept a default-persona fallback, a replaced owner, or an old worker without profile synchronization. */
function checked(start, persona, browser) {
  if (
    browser.apiKey !== start.token ||
    browser.persona?.id !== persona.id ||
    browser.driver.kind !== 'oya' ||
    !browser.profileSync
  )
    throw Error('Native gateway enrollment identity mismatch');
  return browser;
}

/** Return the exact native endpoint with the cleanup descriptor installed by sandbox allocation. */
async function connect(start, persona, browser) {
  const endpoint = browser.driver.cdpEndpoint();
  if (!endpoint) throw Error('Native gateway compatibility adapter is unavailable');
  const cleanup = await cleanupFor(start);
  const upstream = await endpoint.open();
  return { upstream, target: targetFor(start, persona, browser, endpoint, cleanup) };
}

/** The provider result carries native capture separately from the external compatibility endpoint. */
function targetFor(start, persona, browser, endpoint, cleanup) {
  return {
    endpoint,
    cleanup,
    nativeBrowserId: start.reservation.id,
    capture: () => capture(start, persona, browser),
    release: () => retire(start, persona),
  };
}

/** A correlated native flush must finish before its ordered updates become the next cold-start snapshot. */
async function capture(start, persona, browser) {
  assertCurrent(start, persona, browser);
  await browser.driver.cookies();
  assertCurrent(start, persona, browser);
  if (start.profileName) await saveNativeProfile(start, persona);
}

/** Reconnection or persona changes must not retarget profile capture. */
function assertCurrent(start, persona, browser) {
  if (registry.get(start.reservation.id) !== browser) throw Error('Native profile browser was replaced');
  checked(start, persona, browser);
}

/** Destroy only the allocated sandbox; unnamed profiles are disposable once their native process has gone. */
async function retire(start, persona) {
  const cleanup = await cleanupFor(start);
  const removed = await removeSandbox(start.reservation.id, start.token, cleanup?.runtime);
  if (!removed && registry.get(start.reservation.id)) throw Error('Native gateway worker is still connected');
  if (!start.profileName) {
    container.personas.release(persona, start.reservation.id);
    container.personas.remove(start.token, persona.id);
    await container.personas.drain();
  }
}

/** The public session view deliberately hides cleanup; read the owned internal descriptor. */
async function cleanupFor(start) {
  const record = await control().store.get('session', start.reservation.id);
  if (record?.project !== projectId(start.token)) throw Error('Native gateway reservation is unavailable');
  return record.cleanup;
}
