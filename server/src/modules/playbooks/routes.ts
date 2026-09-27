/**
 * REST routes: playbooks and background runs. Each validates, calls the service
 * and answers; the checks live in requests.ts and the run's work in jobs.ts.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { fingerprint } from '../../platform/audit.ts';
import { enforce } from '../../platform/limits.ts';
import { Status } from '../../platform/http-status.ts';
import * as playbooks from './service.ts';
import { track } from '../telemetry/index.ts';
import * as runs from './runs.ts';
import { saveRefusal, recordedRun, playable, runnable } from './requests.ts';
import { runJob } from './jobs.ts';
import { quietCheckpointFor } from './checkpoint.ts';
import { getKey, longJson, requireBrowser } from '../../app/http.ts';

/** Playbook and run routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/**
 * POST /browsers/:browserId/playbooks, save this browser's last ask() by name, then replay it without the LLM.
 * With `steps` in the body, the browser recorded a person doing the task instead:
 * same schema, same storage, same Playwright export.
 */
router.post('/browsers/:browserId/playbooks', authMiddleware, enforce('chat'), requireBrowser, async (req, res) => {
  const body = req.body || {};
  const error = saveRefusal(body);
  if (error) return res.status(Status.BAD_REQUEST).json({ error });
  const run = recordedRun(body);
  const saved = await playbooks.create(getKey(req), req.params.browserId, body.name, run);
  track.playbookSaved(getKey(req), { steps: saved.steps });
  res.json(saved);
});

/** POST /browsers/:browserId/playbooks/:name/play, replay a saved playbook with its variables, healing broken steps unless autoHeal is false. */
router.post(
  '/browsers/:browserId/playbooks/:name/play',
  authMiddleware,
  enforce('chat'),
  requireBrowser,
  async (req, res) => {
    req.setTimeout(0);
    res.setTimeout(0);
    const { browserId, name } = req.params;
    const variables = req.body?.variables ?? {};
    const { pb, status, error } = playable(getKey(req), name, variables);
    if (error) return res.status(status).json({ error });
    // The same walls a chat clears itself: a replay signs in with the profile's saved login and factor.
    const options = { autoHeal: req.body?.autoHeal !== false, checkpoint: quietCheckpointFor(getKey(req), browserId) };
    await longJson(res, () => playbooks.play(getKey(req), browserId, pb, variables, options));
  },
);

/** GET /playbooks, the caller's saved playbooks. */
router.get('/playbooks', authMiddleware, (req, res) => {
  res.json({ playbooks: playbooks.list(getKey(req)) });
});

/** DELETE /playbooks/:name, delete a playbook. */
router.delete('/playbooks/:name', authMiddleware, async (req, res) => {
  await playbooks.remove(getKey(req), req.params.name);
  res.json({ ok: true });
});

/** GET /playbooks/:name/export, the playbook as one JSON document another environment can import. */
router.get('/playbooks/:name/export', authMiddleware, (req, res) => {
  res.json(playbooks.exportPlaybook(getKey(req), req.params.name));
});

/** POST /playbooks/import, save an exported playbook here: `{ playbook: <export>, name?, overwrite? }`. */
router.post('/playbooks/import', authMiddleware, async (req, res) => {
  const { playbook, name, overwrite } = req.body || {};
  res.json(await playbooks.importPlaybook(getKey(req), playbook, { name, overwrite: overwrite === true }));
});

/** POST /playbooks/answer, a free-text field written by the key's model: `{ question, task?, values? }` → `{ answer }`. */
router.post('/playbooks/answer', authMiddleware, enforce('chat'), async (req, res) => {
  const { question, task, values } = req.body || {};
  if (typeof question !== 'string' || !question.trim())
    return res.status(Status.BAD_REQUEST).json({ error: 'question is required' });
  res.json({ answer: await playbooks.answerField(getKey(req), question, { task, values }) });
});

/** PATCH /playbooks/:name, rename a playbook; its healed draft moves with it. */
router.patch('/playbooks/:name', authMiddleware, async (req, res) => {
  res.json(await playbooks.rename(getKey(req), req.params.name, req.body?.name));
});

/** POST /playbooks/:name/promote, make a healed draft the playbook. */
router.post('/playbooks/:name/promote', authMiddleware, async (req, res) => {
  res.json(await playbooks.promote(getKey(req), req.params.name));
});

/** POST /browsers/:browserId/runs, submit a prompt or playbook in the background; the SDK polls and fires callbacks. */
router.post('/browsers/:browserId/runs', authMiddleware, enforce('chat'), requireBrowser, async (req, res) => {
  const { browserId } = req.params;
  const { prompt, playbook, data = {}, secrets = {}, autoHeal = true } = req.body || {};
  const key = getKey(req);
  const { pb, status, error } = runnable(key, { prompt, playbook, data, secrets });
  if (error) return res.status(status).json({ error });
  const job = { key, browserId, pb, prompt, data, secrets, autoHeal };
  const run = runs.start(key, browserId, ({ requestHuman }) => runJob(job, requestHuman));
  res.status(Status.ACCEPTED).json(run);
});

/** GET /runs/:runId, a run's status and result. */
router.get('/runs/:runId', authMiddleware, (req, res) => {
  const run = runs.get(fingerprint(getKey(req)), req.params.runId);
  if (!run) return res.status(Status.NOT_FOUND).json({ error: 'No such run' });
  res.json(run);
});

/** POST /runs/:runId/respond, answer a run that is waiting for a person; 409 if it is not. */
router.post('/runs/:runId/respond', authMiddleware, (req, res) => {
  if (!runs.respond(fingerprint(getKey(req)), req.params.runId, req.body?.response)) {
    return res.status(Status.CONFLICT).json({ error: 'This run is not waiting for a person' });
  }
  res.json({ ok: true });
});
