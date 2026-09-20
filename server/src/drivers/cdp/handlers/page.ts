/**
 * Reading and changing the page: screenshots, the analyzer, form values,
 * waiting for elements, scrolling to an edge, cookies and raw evaluation.
 */
import { elementSelector } from '../browser-scripts.ts';
import { ANALYZE_JS, SCROLL_TO_JS, SET_VALUE_JS } from '../page-scripts.ts';
import queries from '../../../../../browser/scripts/page-queries.cjs';
import { SCREENSHOT_QUALITY, WAIT_TIMEOUT_MS, WAIT_POLL_MS } from '../constants.ts';
import type { Handler } from './types.ts';
import pageRender from '../../../../../browser/scripts/page-render.cjs';

/** A JPEG of the viewport, as a data URL. */
export const screenshot: Handler = async (driver, params, remaining) => {
  const shot = { format: 'jpeg', quality: params.quality || SCREENSHOT_QUALITY };
  const { data } = await driver.conn.send('Page.captureScreenshot', shot, driver.sessionId, remaining());
  return { ok: true, data: { screenshot: `data:image/jpeg;base64,${data}` } };
};

/** The analyzer's structured view of the page. */
export const analyze: Handler = async (driver, params) => {
  await driver.ensureAnalyzer();
  // Written in the format asked for (markdown unless configured), as the desktop app does.
  return pageRender.withPage(await driver.evaluate(ANALYZE_JS(params)), params?.format);
};

/** The visible elements matching a selector, in the page and its iframes, with its URL and title. */
export const readPage: Handler = async (driver, params) => {
  const read = await driver.evaluate(queries.readElementsJs(params.selector, params.limit)).catch(() => null);
  return read || { ok: true, data: { ...(await driver.pageInfo()), elements: [] } };
};

/** Sets a select or input to a value and fires change. */
export const select: Handler = async (driver, params) => {
  if (!params.element_id) return { ok: false, error: 'element_id required' };
  await driver.ensureAnalyzer();
  const ok = await driver.evaluate(SET_VALUE_JS(elementSelector(params.element_id), String(params.value ?? '')));
  return ok ? { ok: true } : { ok: false, error: 'Element not found' };
};

/** Polls until an element (analyzer id or CSS) is present, or the wait runs out. */
export const wait: Handler = async (driver, params, remaining) => {
  const until = Date.now() + Math.min(params.timeout || WAIT_TIMEOUT_MS, remaining());
  await driver.ensureAnalyzer();
  while (Date.now() < until) {
    const found = await driver.evaluate(queries.presentJs(params.selector)).catch(() => false);
    if (found) return { ok: true, data: { found: true } };
    await new Promise((r) => setTimeout(r, WAIT_POLL_MS));
  }
  return { ok: false, error: 'Timeout' };
};

/** Scrolls to the top of the page. */
export const scrollTop: Handler = async (driver) => {
  await driver.evaluate(SCROLL_TO_JS('0'));
  return { ok: true };
};

/** Scrolls to the bottom of the page. */
export const scrollBottom: Handler = async (driver) => {
  await driver.evaluate(SCROLL_TO_JS('document.body.scrollHeight'));
  return { ok: true };
};

/**
 * Server-internal only: challenge handling needs to run its own scripts. Not
 * reachable from the public command API, which is why arbitrary evaluate was
 * removed from that surface. Main world on purpose: this is the channel
 * challenge handling uses.
 */
export const evaluateRaw: Handler = async (driver, params) => ({
  ok: true,
  data: { result: await driver.evaluateMain(String(params.expression || '')) },
});

/** Every cookie the browser holds. */
export const cookies: Handler = async (driver) => ({
  ok: true,
  data: await driver.conn.send('Network.getAllCookies', {}, driver.sessionId),
});
