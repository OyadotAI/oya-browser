import { Router } from 'express';
import { inviteMember, removeMember } from './membership.js';
import { authMiddleware } from '../auth.js';
import { control, hash, fault, projectId } from './service.js';
import { assertSafeTarget } from '../net-guard.js';
import { registry } from '../connection-registry.js';
import { sendCommand } from '../ws-handler.js';
import * as flow from '../flow-recorder.js';

export const controlRouter = Router();
controlRouter.use(authMiddleware);
const key = req => req.principal.key;
const admin = (req, res, next) => req.principal.role === 'administrator' ? next() : res.status(403).json({ error: 'Administrator permission required' });
const wrap = fn => async (req, res, next) => { try { await fn(req, res); } catch (e) { next(e); } };
controlRouter.get('/', wrap(async (req, res) => {
  const view = await control().read(key(req));
  if (req.principal.role !== 'administrator') { delete view.credentials; delete view.webhooks; delete view.deliveries; }
  res.json(view);
}));
controlRouter.post('/sessions', wrap(async (req, res) => {
  const { router } = await import('../api.js');
  req.url = '/browsers/start';
  router.handle(req, res, error => { if (error) res.status(503).json({ error: 'Session creation failed' }); });
}));
controlRouter.post('/sessions/:id/cancel', wrap(async (req, res) => res.json(await control().cancel(key(req), req.params.id))));
controlRouter.get('/sessions', wrap(async (req, res) => res.json(await control().sessions(key(req)))));
controlRouter.get('/sessions/:id', wrap(async (req, res) => res.json(await control().session(key(req), req.params.id))));
controlRouter.patch('/project', admin, wrap(async (req, res) => res.json(await control().settings(key(req), req.body))));
controlRouter.get('/members', admin, wrap(async (req, res) => {
  const project = await control().project(key(req));
  res.json({ owner: project.ownerUser || null, members: await control().store.list('membership', { project: project.id }) });
}));
controlRouter.post('/members/invite', admin, wrap(async (req, res) => res.status(201).json(await inviteMember(key(req), req.body?.role || 'operator'))));
controlRouter.delete('/members/:id', admin, wrap(async (req, res) => res.json(await removeMember(key(req), req.params.id))));
controlRouter.post('/credentials', admin, wrap(async (req, res) => res.status(201).json(await control().credential(key(req), req.body))));
controlRouter.delete('/credentials/:id', admin, wrap(async (req, res) => res.json(await control().revoke(key(req), req.params.id))));
// A shareable, expiring, revocable credential scoped to one live browser. `control: true`
// lets the holder take over and act; otherwise it is view-only. Revoke via DELETE /credentials/:id.
controlRouter.post('/sessions/:id/share', admin, wrap(async (req, res) => res.status(201).json(await control().share(key(req), { id: req.params.id, control: req.body?.control === true, expiresIn: req.body?.expiresIn }))));
controlRouter.post('/sessions/:id/ticket', wrap(async (req, res) => {
  const x = await control().findSession(key(req), req.params.id);
  if (x?.state !== 'ready') throw fault('not_found', 'Ready session not found', 404);
  const ticket = await control().ticket(key(req), x.id, req.authToken);
  res.json({ ticket, expiresIn: 60 });
}));
controlRouter.post('/sessions/:id/control', wrap(async (req, res) => {
  const holder = hash(req.authToken);
  // Acquisition waits up to ten seconds for in-flight commands to settle rather than failing at once.
  const deadline = Date.now() + (req.body?.action === 'acquire' ? 10000 : 0);
  let state;
  for (;;) {
    try {
      if (registry.get(req.params.id)?.pending) throw fault('commands_pending', 'Wait for in-flight commands to settle before transferring control');
      state = await control().takeover(key(req), req.params.id, req.body?.action, holder, { force: req.body?.force === true });
      break;
    } catch (e) {
      if (e.code !== 'commands_pending' || Date.now() >= deadline) throw e;
      await new Promise(r => setTimeout(r, 200));
    }
  }
  registry.get(req.params.id)?.ws?.send(JSON.stringify({ type: 'control_mode', mode: state.mode }));
  res.json(state);
}));
controlRouter.post('/sessions/:id/input', wrap(async (req, res) => {
  const browser = registry.get(req.params.id);
  if (!browser || browser.apiKey !== key(req)) throw fault('not_found', 'Browser not connected', 404);
  if (!['click', 'type', 'press_key', 'scroll', 'click_coordinates', 'double_click', 'drag', 'mouse_move', 'scroll_at', 'type_text', 'keyboard_type', 'navigate', 'back', 'forward', 'reload', 'screenshot', 'analyze', 'read_page'].includes(req.body?.action)) throw fault('invalid_action', 'Unsupported human input', 400);
  const result = await sendCommand(req.params.id, req.body.action, req.body.params || {}, undefined, hash(req.authToken));
  // A recording in progress keeps the navigations the person asked for in the live
  // view; what they click and type is seen in the page itself.
  if (result?.ok !== false) flow.noteCommand(req.params.id, req.body.action, req.body.params || {});
  res.json(result);
}));
/**
 * Recording a flow the person demonstrates here. It runs under the same takeover as
 * their clicks, so the poll is not an agent command competing with them for the browser.
 */
controlRouter.post('/sessions/:id/record', wrap(async (req, res) => {
  const browser = registry.get(req.params.id);
  if (!browser || browser.apiKey !== key(req)) throw fault('not_found', 'Browser not connected', 404);
  const holder = hash(req.authToken);
  const dispatch = (action, params) => sendCommand(req.params.id, action, params, undefined, holder);
  const mode = req.body?.mode;
  if (mode === 'start') return res.json(await flow.start(req.params.id, dispatch));
  if (mode === 'stop') {
    const final = (await flow.stop(req.params.id, dispatch)) || (await flow.status(req.params.id));
    if (req.body?.resume === true) {
      await control().takeover(key(req), req.params.id, 'release', holder);
      await control().takeover(key(req), req.params.id, 'resume', holder);
    }
    return res.json(final);
  }
  if (mode === 'discard') return res.json(await flow.discard(req.params.id));
  if (mode === 'status') return res.json(await flow.status(req.params.id, dispatch));
  throw fault('invalid_mode', 'mode must be start, stop, status or discard', 400);
}));
controlRouter.post('/sessions/:id/stop', wrap(async (req, res) => {
  const { stopBrowser } = await import('../api.js');
  const result = await stopBrowser(req, req.params.id, { force: req.body?.force === true });
  res.status(result.ok ? 200 : 409).json(result);
}));
controlRouter.post('/sessions/:id/recover', wrap(async (req, res) => {
  const x = await control().session(key(req), req.params.id);
  if (x.state === 'ready') return res.json({ outcome: 'original_session', session: x });
  if (req.body?.replace !== true) throw fault('recovery_unavailable', 'Original browser is unavailable. Explicit replacement is required; prior command outcomes may be unknown', 409);
  if (!['stopped', 'failed'].includes(x.state)) throw fault('cleanup_required', 'Stop the original resource and confirm cleanup before replacing it');
  const { router } = await import('../api.js');
  if (x.provider === 'cdp' && !req.body?.wsUrl) throw fault('replacement_endpoint_required', 'Supply wsUrl for a replacement CDP browser', 422);
  req.recoveryPolicies = x.policies || [];
  req.body = { provider: x.provider, persona: x.persona, governed: !!x.runtime, replacementOf: x.id, ...(req.body?.wsUrl ? { wsUrl: req.body.wsUrl } : {}), ...(x.budgetUsd != null ? { budgetUsd: x.budgetUsd } : {}) };
  req.url = '/browsers/start';
  const json = res.json.bind(res);
  res.json = value => json({ ...value, recovery: { outcome: 'replacement_from_profile', previousSessionId: x.id, previousCommandOutcome: 'unknown' } });
  router.handle(req, res, error => { if (error) res.status(503).json({ error: 'Replacement failed' }); });
}));
controlRouter.get('/events', wrap(async (req, res) => {
  const since = Number(req.query.after || 0);
  if (!Number.isSafeInteger(since) || since < 0) throw fault('invalid_cursor', 'Invalid event cursor', 400);
  const events = await control().events(key(req), { after: since, limit: 500 });
  res.json({ events, cursor: events.at(-1)?.id ?? since });
}));
controlRouter.get('/audit/export', admin, wrap(async (req, res) => {
  res.type('application/x-ndjson');
  // Paged, so a long audit window is never held in memory at once.
  for (let after = 0, page; (page = await control().events(key(req), { after, limit: 1000 })).length; after = page.at(-1).id) res.write(page.map(e => JSON.stringify(e)).join('\n') + '\n');
  res.end();
}));
controlRouter.post('/webhooks', admin, wrap(async (req, res) => {
  await assertSafeTarget(req.body?.url, { protocols: ['https:'], label: 'webhook URL' });
  res.status(201).json(await control().webhook(key(req), req.body));
}));
controlRouter.delete('/webhooks/:id', admin, wrap(async (req, res) => {
  await control().store.transact(async tx => {
    const h = await tx.get('webhook', req.params.id);
    if (!h || h.project !== projectId(key(req))) throw fault('not_found', 'Webhook not found', 404);
    h.enabled = false;
  });
  res.json({ ok: true });
}));
controlRouter.post('/deliveries/:id/replay', admin, wrap(async (req, res) => {
  await control().store.transact(async tx => {
    const d = await tx.get('delivery', req.params.id);
    if (!d || d.project !== projectId(key(req))) throw fault('not_found', 'Delivery not found', 404);
    Object.assign(d, { state: 'pending', attempts: 0, nextAt: Date.now(), replayAt: Date.now() });
  });
  res.json({ ok: true });
}));
controlRouter.use((err, req, res, next) => { if (res.headersSent) return next(err); res.status(err.status || 503).json({ error: err.message, code: err.code || 'control_unavailable' }); });
