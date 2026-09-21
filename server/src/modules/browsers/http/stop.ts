/**
 * Routes that take browsers off the fleet: stop one or many, force a
 * disconnect, or detach a CDP browser.
 */
import { registry } from '../registry.ts';
import { listSandboxBrowsers } from '../../../drivers/sandbox.ts';
import { audit, fingerprint } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';
import { canAccess, getKey } from '../../../app/http.ts';
import { sessions as gatewaySessions } from '../../gateway/service.ts';
import { control, live } from '../../control/service.ts';
import { stopBrowser } from '../lifecycle/stop.ts';
import { countBrowsers, dropBrowser } from '../lifecycle/fleet.ts';
import { MAX_STOP_IDS, STOP_BATCH } from '../constants.ts';
import { auditBrowser } from './helpers.ts';

/** Stops one browser; answers 200, or the failure's status (404 when it has none). */
export async function stopOne(req, res) {
  const options = { sandbox: req.body?.sandbox, force: req.body?.force === true };
  const result = await stopBrowser(req, req.params.browserId, options);
  res.status(result.ok ? Status.OK : result.status || Status.NOT_FOUND).json(result);
}

/** Stops `{ids: [...]}` or `{all: true}`; each id reports separately. */
export async function stopMany(req, res) {
  const all = req.body?.all === true;
  const ids = all ? await everyBrowserId(getKey(req)) : requestedIds(req.body);
  if (!ids.length && all) return res.json({ ok: true, stopped: 0, results: [] });
  if (!ids.length) return res.status(Status.BAD_REQUEST).json({ error: 'Pass ids: [...] or all: true' });
  const results = await stopInBatches(req, ids);
  res.json({ ok: true, stopped: results.filter((r) => r.ok).length, results });
}

/** Every browser the key has anywhere: sandboxes, gateway sessions and durable sessions. */
async function everyBrowserId(key) {
  const sandboxes = (await listSandboxBrowsers(key, registry.list(key))).map((b) => b.id);
  const gateway = [...gatewaySessions.values()].filter((s) => s.apiKey === key && !s.attachedTo).map((s) => s.id);
  const durable = (await control().sessions(key, live)).map((s) => s.id);
  return [...new Set([...sandboxes, ...gateway, ...durable])];
}

/** The ids the caller listed, as strings, capped. */
const requestedIds = (body) => (Array.isArray(body?.ids) ? body.ids.map(String).slice(0, MAX_STOP_IDS) : []);

/**
 * Sandboxes are deleted over the network; a few at a time keeps a 1k-browser
 * "stop all" from opening a thousand connections to Oya Cloud at once.
 */
async function stopInBatches(req, ids: string[]) {
  const results = [];
  for (let i = 0; i < ids.length; i += STOP_BATCH)
    results.push(...(await Promise.all(ids.slice(i, i + STOP_BATCH).map((id) => stopBrowser(req, id)))));
  return results;
}

/** Forces one browser off the fleet, a stuck client, a runaway, an abusive key. */
export function disconnect(req, res) {
  const { browserId } = req.params;
  const browser = registry.get(browserId);
  if (!browser || !canAccess(req, browserId)) return res.status(Status.NOT_FOUND).json(NOT_CONNECTED);
  dropBrowser(browserId, browser.ws, 'Disconnected by operator');
  const meta = { clientType: browser.clientType, reason: req.body?.reason || null };
  auditBrowser(req, 'browser.disconnect', browserId, meta);
  countBrowsers();
  res.json({ ok: true, disconnected: browserId });
}

/** The 404 body for a browser the caller cannot reach. */
const NOT_CONNECTED = { error: 'Browser not connected' };

/** Drops every browser on the calling key, without waiting for key deletion. */
export function disconnectAll(req, res) {
  const target = getKey(req);
  const ids = [...registry.browsers.entries()].filter(([, b]) => b.apiKey === target).map(([id]) => id);
  for (const id of ids) dropBrowser(id, registry.get(id)?.ws, 'Key disconnected by operator');
  auditKeyDisconnect(req, target, ids.length);
  countBrowsers();
  res.json({ ok: true, disconnected: ids.length });
}

/** Audits dropping `count` browsers of the key `target`. */
function auditKeyDisconnect(req, target, count: number) {
  const actorKey = getKey(req);
  const targetId = fingerprint(target);
  audit({ action: 'key.disconnect', actorKey, targetType: 'key', targetId, meta: { browsers: count }, req });
}

/** Detaches a CDP browser and releases the vendor session. */
export async function detach(req, res) {
  const { browserId } = req.params;
  const browser = registry.get(browserId);
  if (!browser || !canAccess(req, browserId)) return res.status(Status.NOT_FOUND).json({ error: 'Browser not found' });
  const result = await stopBrowser(req, browserId);
  if (!result.ok) return res.status(result.status || Status.NOT_FOUND).json(result);
  auditBrowser(req, 'browser.disconnect', browserId, { clientType: browser.clientType, provider: browser.provider });
  res.json({ ok: true });
}
