/**
 * Health is derived, never stored: a browser that has not been heard from in
 * 40s is stale, in 80s dead, and one that failed 3 of its last 10 commands
 * is in trouble regardless of heartbeat.
 */
import { DEAD_AFTER_MS, FAILING_COMMANDS, HEALTH_WINDOW, STALE_AFTER_MS } from '../constants.ts';

/** Enough of the recent commands failed to call the browser troubled. */
function failing(activity) {
  const recent = activity.slice(0, HEALTH_WINDOW);
  return recent.length >= FAILING_COMMANDS && recent.filter((a) => !a.ok).length >= FAILING_COMMANDS;
}

/** A live browser: 'errors' when its commands are failing, else 'ok'. */
const alive = (isFailing: boolean) => (isFailing ? 'errors' : 'ok');

/** A driver that can say it is dead and does. */
const driverDead = (driver) => typeof driver.isAlive === 'function' && !driver.isAlive();

/** 'ok', 'errors', 'stale' or 'dead' for one browser record. */
export function healthOf(b, now = Date.now()) {
  const isFailing = failing(b.activity);
  // An outbound (CDP) browser has no heartbeat: we drive it, it does not
  // report in. Its socket being open is the liveness signal.
  if (b.driver) return driverDead(b.driver) ? 'dead' : alive(isFailing);
  const silent = now - b.lastSeen.getTime();
  if (silent > DEAD_AFTER_MS) return 'dead';
  if (silent > STALE_AFTER_MS) return 'stale';
  return alive(isFailing);
}
