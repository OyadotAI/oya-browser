/**
 * Moving the page: navigate, reload, and walking the session history.
 */
import { LOAD_WAIT_MAX_MS } from '../constants.ts';
import type { Handler } from './types.ts';

/** Loads a URL (https assumed when no scheme is given) and waits for its load event. */
export const navigate: Handler = async (driver, params, remaining) => {
  if (!params.url) return { ok: false, error: 'URL required' };
  const url = /^https?:\/\//i.test(params.url) ? params.url : `https://${params.url}`;
  const loaded = driver.conn.once('Page.loadEventFired', Math.min(remaining(), LOAD_WAIT_MAX_MS));
  const navigated = await driver.conn.send('Page.navigate', { url }, driver.sessionId, remaining());
  if (navigated.errorText) throw new Error(`Navigation failed: ${navigated.errorText}`);
  await loaded;
  return { ok: true, data: await driver.pageInfo() };
};

/** Reloads the page. */
export const reload: Handler = async (driver, _params, remaining) => {
  await driver.conn.send('Page.reload', {}, driver.sessionId, remaining());
  return { ok: true };
};

/** A handler that moves one entry through the history; `name` is the action, for the error. */
function historyStep(step: number, name: string): Handler {
  return async (driver) => {
    const { currentIndex, entries } = await driver.conn.send('Page.getNavigationHistory', {}, driver.sessionId);
    const target = entries[currentIndex + step];
    if (!target) return { ok: false, error: `Cannot go ${name}` };
    await driver.conn.send('Page.navigateToHistoryEntry', { entryId: target.id }, driver.sessionId);
    return { ok: true };
  };
}

/** One entry back in the history. */
export const back = historyStep(-1, 'back');
/** One entry forward in the history. */
export const forward = historyStep(1, 'forward');
