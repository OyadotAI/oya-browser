/**
 * Live-view viewers of one browser: each is an open SSE response.
 */
import { VIEWER_BACKLOG_BYTES } from '../constants.ts';

/** Sends a frame to every viewer; a viewer too far behind skips it, a broken one is dropped. */
export function sendFrame(browser, dataUrl) {
  for (const res of browser.streamViewers) {
    if (res.writableLength > VIEWER_BACKLOG_BYTES) continue;
    try {
      res.write(`data: ${dataUrl}\n\n`);
    } catch {
      browser.streamViewers.delete(res);
    }
  }
}

/** Ends every viewer's stream, for a browser that is going away. */
export function endViewers(browser) {
  for (const res of browser.streamViewers) {
    try {
      res.end();
    } catch {}
  }
  browser.streamViewers.clear();
}
