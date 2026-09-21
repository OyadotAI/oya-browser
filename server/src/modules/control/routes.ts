/**
 * The control-plane REST API under /control: sessions, human takeover, members,
 * credentials, webhooks and the audit log, scoped to the calling project.
 */
import { Router } from 'express';
import { inviteMember, removeMember } from './membership.ts';
import { authMiddleware } from '../auth/service.ts';
import { control, fault, projectId } from './service.ts';
import { desktopState } from './desktop.ts';
import { assertSafeTarget } from '../../platform/net-guard.ts';
import { registry } from '../browsers/registry.ts';
import { Status } from '../../platform/http-status.ts';
import { key, admin, requireOwnBrowser, redispatchStart } from './http/guards.ts';
import { transferControl } from './http/takeover.ts';
import { humanInput } from './http/input.ts';
import { recordFlow } from './http/record.ts';
import { recover } from './http/recover.ts';

/** Mounted at /control. */
export const controlRouter = Router();
/** Every control route needs a project credential. */
controlRouter.use(authMiddleware);
/** GET /control, the project overview; credentials, webhooks and deliveries only for administrators. */
controlRouter.get('/', async (req, res) => {
  const view = await control().read(key(req));
  if (req.principal.role !== 'administrator') {
    delete view.credentials;
    delete view.webhooks;
    delete view.deliveries;
  }
  res.json(view);
});
/** POST /control/sessions, start a browser, by re-dispatching to POST /browsers/start. */
controlRouter.post('/sessions', (req, res) => redispatchStart(req, res, 'Session creation failed'));
/** POST /control/sessions/:id/cancel, cancel a session. */
controlRouter.post('/sessions/:id/cancel', async (req, res) =>
  res.json(await control().cancel(key(req), req.params.id)),
);
/** GET /control/sessions, this project's sessions. */
controlRouter.get('/sessions', async (req, res) => res.json(await control().sessions(key(req))));
/** GET /control/sessions/:id, one session, 404 if it is not this project's. */
controlRouter.get('/sessions/:id', async (req, res) => res.json(await control().session(key(req), req.params.id)));
/** PATCH /control/project, update project settings. Administrators only. */
controlRouter.patch('/project', admin, async (req, res) => res.json(await control().settings(key(req), req.body)));
/** GET /control/members, the project owner and memberships. Administrators only. */
controlRouter.get('/members', admin, async (req, res) => {
  const project = await control().project(key(req));
  res.json({
    owner: project.ownerUser || null,
    members: await control().store.list('membership', { project: project.id }),
  });
});
/** POST /control/members/invite, invite a member with a role (operator by default). Administrators only. */
controlRouter.post('/members/invite', admin, async (req, res) =>
  res.status(Status.CREATED).json(await inviteMember(key(req), req.body?.role || 'operator')),
);
/** DELETE /control/members/:id, remove a member. Administrators only. */
controlRouter.delete('/members/:id', admin, async (req, res) => res.json(await removeMember(key(req), req.params.id)));
/** POST /control/credentials, issue a project credential. Administrators only. */
controlRouter.post('/credentials', admin, async (req, res) =>
  res.status(Status.CREATED).json(await control().credential(key(req), req.body)),
);
/** DELETE /control/credentials/:id, revoke a credential. Administrators only. */
controlRouter.delete('/credentials/:id', admin, async (req, res) =>
  res.json(await control().revoke(key(req), req.params.id)),
);
/**
 * POST /control/sessions/:id/share, a shareable, expiring, revocable credential scoped to one live browser.
 * `control: true` lets the holder take over and act; otherwise it is view-only. Revoke via DELETE /credentials/:id.
 * Administrators only.
 */
controlRouter.post('/sessions/:id/share', admin, async (req, res) =>
  res.status(Status.CREATED).json(
    await control().share(key(req), {
      id: req.params.id,
      control: req.body?.control === true,
      expiresIn: req.body?.expiresIn,
    }),
  ),
);
/** POST /control/sessions/:id/ticket, a single-use connection ticket for a ready session, valid 60 seconds. */
controlRouter.post('/sessions/:id/ticket', async (req, res) => {
  const x = await control().findSession(key(req), req.params.id);
  if (x?.state !== 'ready') throw fault('not_found', 'Ready session not found', Status.NOT_FOUND);
  const ticket = await control().ticket(key(req), x.id, req.authToken);
  res.json({ ticket, expiresIn: 60 });
});
/** POST /control/sessions/:id/control, change who drives the session (request, acquire, renew, return, release, resume) and tell the browser. */
controlRouter.post('/sessions/:id/control', async (req, res) => {
  const state = await transferControl(key(req), req);
  registry
    .get(req.params.id)
    ?.ws?.send(JSON.stringify({ type: 'control_mode', mode: state.mode, state: desktopState(req.params.id, state) }));
  res.json(state);
});
/** POST /control/sessions/:id/input, a person's input (click, type, navigate…) sent to the browser under their takeover. */
controlRouter.post('/sessions/:id/input', async (req, res) => {
  requireOwnBrowser(req);
  res.json(await humanInput(req));
});
/**
 * POST /control/sessions/:id/record, start, stop, discard or poll recording a flow the person demonstrates here. It runs under the same takeover as
 * their clicks, so the poll is not an agent command competing with them for the browser.
 */
controlRouter.post('/sessions/:id/record', async (req, res) => {
  requireOwnBrowser(req);
  res.json(await recordFlow(key(req), req));
});
/** POST /control/sessions/:id/stop, stop the browser; 409 when it could not be stopped. */
controlRouter.post('/sessions/:id/stop', async (req, res) => {
  const { stopBrowser } = await import('../../app/api.ts');
  const result = await stopBrowser(req, req.params.id, { force: req.body?.force === true });
  res.status(result.ok ? Status.OK : Status.CONFLICT).json(result);
});
/**
 * POST /control/sessions/:id/recover, the session itself if still ready; otherwise, with `replace: true`,
 * a replacement started from the same profile once the original is stopped or failed.
 */
controlRouter.post('/sessions/:id/recover', recover);
/** GET /control/events, the event log after the `after` cursor, 500 at a time. */
controlRouter.get('/events', async (req, res) => {
  const since = Number(req.query.after || 0);
  if (!Number.isSafeInteger(since) || since < 0)
    throw fault('invalid_cursor', 'Invalid event cursor', Status.BAD_REQUEST);
  const events = await control().events(key(req), { after: since, limit: 500 });
  res.json({ events, cursor: events.at(-1)?.id ?? since });
});
/** GET /control/audit/export, the whole event log as NDJSON. Administrators only. */
controlRouter.get('/audit/export', admin, async (req, res) => {
  res.type('application/x-ndjson');
  // Paged, so a long audit window is never held in memory at once.
  for (
    let after = 0, page;
    (page = await control().events(key(req), { after, limit: 1000 })).length;
    after = page.at(-1).id
  )
    res.write(page.map((e) => JSON.stringify(e)).join('\n') + '\n');
  res.end();
});
/** GET /control/webhook, the Settings endpoint: one per project, edited in place. Administrators only. */
controlRouter.get('/webhook', admin, async (req, res) => res.json(await control().webhookConfig(key(req))));
/** PUT /control/webhook, set the project webhook's URL and event types, optionally rolling its secret. Administrators only. */
controlRouter.put('/webhook', admin, async (req, res) => {
  await assertSafeTarget(req.body?.url, { protocols: ['https:'], label: 'webhook URL' });
  res.json(
    await control().webhook(key(req), { url: req.body.url, types: req.body.types ?? [], roll: req.body.roll === true }),
  );
});
/** DELETE /control/webhook, disable the project webhook. Administrators only. */
controlRouter.delete('/webhook', admin, async (req, res) => {
  await control().store.transact(async (tx) => {
    const h = await tx.get('webhook', `hook:${projectId(key(req))}`);
    if (h) h.enabled = false;
  });
  res.json({ ok: true });
});
/** POST /control/webhooks, add a webhook. Administrators only. */
controlRouter.post('/webhooks', admin, async (req, res) => {
  await assertSafeTarget(req.body?.url, { protocols: ['https:'], label: 'webhook URL' });
  res.status(Status.CREATED).json(await control().webhook(key(req), req.body));
});
/** DELETE /control/webhooks/:id, disable a webhook. Administrators only. */
controlRouter.delete('/webhooks/:id', admin, async (req, res) => {
  await control().store.transact(async (tx) => {
    const h = await tx.get('webhook', req.params.id);
    if (!h || h.project !== projectId(key(req))) throw fault('not_found', 'Webhook not found', Status.NOT_FOUND);
    h.enabled = false;
  });
  res.json({ ok: true });
});
/** POST /control/deliveries/:id/replay, queue a webhook delivery to be sent again. Administrators only. */
controlRouter.post('/deliveries/:id/replay', admin, async (req, res) => {
  await control().store.transact(async (tx) => {
    const d = await tx.get('delivery', req.params.id);
    if (!d || d.project !== projectId(key(req))) throw fault('not_found', 'Delivery not found', Status.NOT_FOUND);
    Object.assign(d, { state: 'pending', attempts: 0, nextAt: Date.now(), replayAt: Date.now() });
  });
  res.json({ ok: true });
});
/** Errors as JSON with a code; 503 control_unavailable when the error carries none. */
controlRouter.use((err, req, res, next) => {
  if (res.headersSent) return next(err);
  res.status(err.status || Status.UNAVAILABLE).json({ error: err.message, code: err.code || 'control_unavailable' });
});
