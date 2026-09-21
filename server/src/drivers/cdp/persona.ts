/**
 * Applies a persona's device identity to a CDP target: the user agent (header
 * and JS property both), client hints, and the browser package's injection.
 */
import { userAgentFor, metadataFor } from '../../modules/personas/ua.ts';
import { getApplier } from './browser-scripts.ts';
import type { CDPDriver } from './driver.ts';

/**
 * Anchor, Browserbase, Steel and Browser Use ship tuned stealth of their own.
 * Layering ours on top produces contradictions that are themselves detectable,
 * so the injection is for browsers nobody else has already treated.
 */
export const PROVIDER_SHIPS_STEALTH = new Set(['anchor', 'browserbase', 'steel', 'browseruse']);

/**
 * The UA is an HTTP header as well as a JS property, so it cannot be fixed
 * from an injected script, a page reads HeadlessChrome from the header no
 * matter what navigator.userAgent says. Emulation sets both.
 */
export async function applyPersona(driver: CDPDriver, sessionId) {
  const userAgent = await userAgentOverride(driver, sessionId);
  // The persona, applied exactly as test-stealth.js measures it: native
  // emulation, the injection, and every worker and cross-site iframe. One
  // applier per connection; service workers are browser-wide, so once.
  const create = getApplier();
  if (create && !driver.personaApply) await createApplier(driver, create, userAgent);
  if (driver.personaApply) await driver.personaApply.page(sessionId);
}

/** The UA override for this browser's real version, or null (with a warning) when it cannot be built. */
async function userAgentOverride(driver: CDPDriver, sessionId) {
  try {
    const { version, brands } = await readBrowser(driver, sessionId);
    driver.userAgent = userAgentFor(driver.fingerprint, version.userAgent);
    return overrideFor(driver, version, brands);
  } catch (e) {
    console.warn(`[cdp] user agent override failed (${e.message}), this browser reports its real UA`);
    return null;
  }
}

/** The browser's version and brand list, in that order. */
async function readBrowser(driver: CDPDriver, sessionId) {
  const version = await driver.conn.send('Browser.getVersion');
  const brands = await readBrands(driver, sessionId);
  return { version, brands };
}

/**
 * Read the browser's own brand list first. The GREASE entry ("Not?A_Brand"
 * and friends) changes between releases, so reusing it beats constructing one,
 * and overriding without any metadata blanks client hints, which is itself a tell.
 */
function readBrands(driver: CDPDriver, sessionId) {
  const expression = 'JSON.stringify(navigator.userAgentData?.brands || [])';
  return driver.conn
    .send('Runtime.evaluate', { expression, returnByValue: true }, sessionId)
    .then(parseBrands)
    .catch(() => []);
}

/** The brand list from an evaluate result; empty when it does not parse. */
function parseBrands(r) {
  try {
    return JSON.parse(r.result?.value || '[]');
  } catch {
    return [];
  }
}

/** Emulation.setUserAgentOverride's arguments for the persona. */
function overrideFor(driver: CDPDriver, version, brands) {
  return {
    userAgent: driver.userAgent,
    ...(driver.acceptLanguage ? { acceptLanguage: driver.acceptLanguage } : {}),
    platform: driver.fingerprint.navigator?.platform || undefined,
    userAgentMetadata: metadataFor(driver.fingerprint, version.userAgent, brands),
  };
}

/** Binds a persona applier to the driver's connection and applies the browser-wide part once. */
async function createApplier(driver: CDPDriver, create, userAgent) {
  driver.personaApply = create({
    send: (method, params, sid) => driver.conn.send(method, params, sid),
    on: (event, fn) => driver.conn.on(event, fn),
    profile: driver.fingerprint,
    userAgent,
    onError: (what, err) => console.warn(`[cdp] ${what} failed, not covered: ${err.message}`),
  });
  await driver.personaApply.browser();
}
