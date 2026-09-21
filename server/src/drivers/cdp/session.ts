/**
 * Getting a driver onto a page: connecting, choosing or creating a target,
 * and preparing each target it attaches to.
 */
import { randomBytes } from 'crypto';
import { createRequire } from 'module';
import { CDPConnection } from './connection.ts';
import { PROVIDER_SHIPS_STEALTH, applyPersona } from './persona.ts';
import { TAG_BYTES } from './constants.ts';
import type { CDPDriver } from './driver.ts';

const require = createRequire(import.meta.url);
const { cdpCookies } = require('../../../../browser/login-state.js');

/** CDP domains every attached target runs with. */
const DOMAINS = ['Page', 'Runtime', 'DOM', 'Network'];

/** Connects, attaches to a page (creating one if none), and restores saved cookies. */
export async function connectDriver(driver: CDPDriver) {
  driver.conn = await new CDPConnection(driver.wsUrl).connect();
  driver.personaApply = null; // bound to the connection it listens on
  notifyOnClose(driver);
  await driver.attach(await firstPage(driver));
  if (driver.login?.cookies?.length)
    await driver.conn.send('Network.setCookies', { cookies: cdpCookies(driver.login.cookies) }, driver.sessionId);
  return driver;
}

/** Tells the driver's owner when the CDP socket closes; a throwing callback is ignored. */
function notifyOnClose(driver: CDPDriver) {
  driver.conn.ws.on('close', () => {
    try {
      driver.onClose?.();
    } catch {}
  });
}

/** Attach to a page target, creating one if the browser has none. */
async function firstPage(driver: CDPDriver) {
  const { targetInfos = [] } = await driver.conn.send('Target.getTargets');
  const page = targetInfos.find((t) => t.type === 'page');
  if (page) return page.targetId;
  const { targetId } = await driver.conn.send('Target.createTarget', { url: 'about:blank' });
  return targetId;
}

/** Attaches to a target and prepares it: domains, dialogs, persona, recording and login state. */
export async function attachTarget(driver: CDPDriver, targetId) {
  await stopRecordChannel(driver);
  const { sessionId } = await driver.conn.send('Target.attachToTarget', { targetId, flatten: true });
  Object.assign(driver, { sessionId, targetId, analyzerLoaded: false });
  for (const domain of DOMAINS) await driver.conn.send(`${domain}.enable`, {}, sessionId).catch(() => {});
  // Page is enabled just above, which means Chromium routes every alert(),
  // confirm() and prompt() to us and blocks the renderer until we answer. With
  // no listener the tab wedges and every later command eats its whole timeout.
  driver.watchDialogs();
  // A vendor that ships tuned stealth owns the whole surface: our UA,
  // timezone and patches together would contradict theirs, and a
  // contradiction is a stronger signal than either alone. The persona still
  // governs that session's cookie jar, proxy and concurrency there, only
  // the device spoofing is theirs to do.
  if (driver.fingerprint && !PROVIDER_SHIPS_STEALTH.has(driver.provider)) await applyPersona(driver, sessionId);
  await prepareWorld(driver, sessionId);
}

/** A recording streams from one target; stop it before leaving that target. */
async function stopRecordChannel(driver: CDPDriver) {
  if (!driver.recordChannel) return;
  await driver.recordChannel.stop();
  driver.recordChannel = null;
}

/**
 * The analyzer lives in an isolated world, never the page's. In the main
 * world its globals, analyzePage, __acFindElement, __acAnalyzerLoaded, are
 * a one-line, 100%-precision detector for this product, which is worth more
 * to a defender than every other signal on the page combined. Same
 * per-session random tag attribute as the desktop path.
 */
async function prepareWorld(driver: CDPDriver, sessionId) {
  driver.tagAttr = 'data-' + randomBytes(TAG_BYTES).toString('hex');
  driver.worldContext = null;
  if (driver.recording) await driver.armRecording();
  if (driver.loginState)
    await driver.loginState.attach(sessionSender(driver, sessionId), sessionListener(driver, sessionId));
}

/** Sends CDP commands into one target session. */
export function sessionSender(driver: CDPDriver, sessionId) {
  return (method, params = {}) => driver.conn.send(method, params, sessionId);
}

/** Subscribes to a CDP event, but only as it happens in one target session. */
export function sessionListener(driver: CDPDriver, sessionId) {
  return (method, fn) =>
    driver.conn.on(method, (params, sid) => {
      if (sid === sessionId) fn(params);
    });
}
