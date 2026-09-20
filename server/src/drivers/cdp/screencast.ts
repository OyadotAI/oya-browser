/**
 * The live view for a CDP browser. Frames are pushed the same way the Oya
 * client does, so /live/:id is identical for both client types. CDP
 * screencasts natively — no polling loop.
 */
import {
  SCREENCAST_QUALITY,
  SCREENCAST_MAX_WIDTH,
  SCREENCAST_EVERY_NTH_FRAME,
  IDLE_FILL_INTERVAL_MS,
  IDLE_FRAME_GAP_MS,
  IDLE_FILL_TIMEOUT_MS,
} from './constants.ts';
import type { CDPDriver } from './driver.ts';

/** When the last frame went out, and whether an idle-fill screenshot is in flight. */
interface FillState {
  /** Time of the last frame sent, from either source. */
  lastFrameAt: number;
  /** An idle-fill screenshot is being taken. */
  busy: boolean;
}

/** Page.startScreencast's arguments, defaults filled in. */
function castSettings({
  quality = SCREENCAST_QUALITY,
  maxWidth = SCREENCAST_MAX_WIDTH,
  everyNthFrame = SCREENCAST_EVERY_NTH_FRAME,
}: any) {
  return { format: 'jpeg', quality, maxWidth, everyNthFrame };
}

/** Starts the screencast and its idle fill; a second start is a no-op. */
export async function startScreencast(driver: CDPDriver, onFrame, options: any = {}) {
  const settings = castSettings(options);
  if (driver.screencasting) return;
  driver.screencasting = true;
  const state: FillState = { lastFrameAt: 0, busy: false };
  driver.offScreencast = subscribeFrames(driver, onFrame, state);
  await driver.conn.send('Page.startScreencast', settings, driver.sessionId);
  // Chrome only screencasts on repaint. A page that is sitting still sends
  // nothing — so the live view of an idle browser would be blank forever,
  // which reads as "broken", not "idle". Fill the gaps with a screenshot
  // about once a second; the screencast takes over the moment anything moves.
  driver.screencastFill = setInterval(() => fillIdle(driver, onFrame, state, settings.quality), IDLE_FILL_INTERVAL_MS);
}

/** Listens for screencast frames; returns the unsubscribe. */
function subscribeFrames(driver: CDPDriver, onFrame, state: FillState) {
  return driver.conn.on('Page.screencastFrame', (params, sessionId) =>
    forwardFrame(driver, onFrame, state, params, sessionId),
  );
}

/** Passes one screencast frame on and acknowledges it. */
async function forwardFrame(driver: CDPDriver, onFrame, state: FillState, params, sessionId) {
  if (sessionId && sessionId !== driver.sessionId) return;
  state.lastFrameAt = Date.now();
  try {
    onFrame(`data:image/jpeg;base64,${params.data}`);
  } catch {}
  // Must ack or Chrome stops sending.
  driver.conn.send('Page.screencastFrameAck', { sessionId: params.sessionId }, driver.sessionId).catch(() => {});
}

/** Sends a screenshot when the screencast has been quiet a while. */
async function fillIdle(driver: CDPDriver, onFrame, state: FillState, quality) {
  if (state.busy || !driver.screencasting || Date.now() - state.lastFrameAt < IDLE_FRAME_GAP_MS) return;
  state.busy = true;
  try {
    await idleFrame(driver, onFrame, quality);
  } catch {
    /* the next tick tries again */
  } finally {
    state.busy = false;
  }
}

/** One screenshot for the live view, sent only if the screencast is still on. */
async function idleFrame(driver: CDPDriver, onFrame, quality) {
  const params = { format: 'jpeg', quality, optimizeForSpeed: true };
  const { data } = await driver.conn.send('Page.captureScreenshot', params, driver.sessionId, IDLE_FILL_TIMEOUT_MS);
  if (driver.screencasting) onFrame(`data:image/jpeg;base64,${data}`);
}

/** Stops the live-view screencast and its idle fill. */
export async function stopScreencast(driver: CDPDriver) {
  if (!driver.screencasting) return;
  driver.screencasting = false;
  clearInterval(driver.screencastFill);
  driver.offScreencast?.();
  await driver.conn.send('Page.stopScreencast', {}, driver.sessionId).catch(() => {});
}
