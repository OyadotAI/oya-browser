/**
 * Routes that look at browsers: one browser's detail and its live view.
 */
import { registry } from '../registry.ts';
import { listSandboxBrowsers } from '../../../drivers/sandbox.ts';
import { Status } from '../../../platform/http-status.ts';
import { canAccess, getKey } from '../../../app/http.ts';
import { browserCdpUrl } from '../lifecycle/cdp-url.ts';

/** One browser with its recent activity; a CDP URL too, for anyone but a viewer. */
export async function browserDetail(req, res) {
  const { browserId } = req.params;
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) return cloudDetail(req, res, browserId);
  const detail: any = registry.describe(browserId);
  const offersCdp = detail.clientType === 'cdp' || registry.get(browserId)?.cdp;
  if (offersCdp && req.principal?.role !== 'viewer') detail.cdpUrl = await browserCdpUrl(req, browserId);
  res.json(detail);
}

/** A browser not held here may still be the key's Oya Cloud sandbox; else 404. */
async function cloudDetail(req, res, browserId) {
  const cloud = (await listSandboxBrowsers(getKey(req))).find((row) => row.id === browserId);
  if (cloud) return res.json({ ...cloud, activity: [] });
  return res.status(Status.NOT_FOUND).json({ error: `Browser ${browserId} not connected` });
}

/** Headers for a server-sent event stream. */
const SSE_HEADERS = { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache', Connection: 'keep-alive' };

/** Streams a browser's frames to this response until the viewer goes away. */
export function liveView(req, res) {
  const { browserId } = req.params;
  res.writeHead(Status.OK, SSE_HEADERS);
  sendLatestFrame(res, registry.get(browserId));
  res.authToken = req.authToken;
  registry.addViewer(browserId, res);
  req.on('close', () => registry.removeViewer(browserId, res));
}

/** Sends the latest frame immediately, if there is one. */
function sendLatestFrame(res, browser) {
  if (!browser?.lastFrame) return;
  try {
    res.write(`data: ${browser.lastFrame}\n\n`);
  } catch {}
}
