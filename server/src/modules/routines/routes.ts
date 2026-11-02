/**
 * REST routes: a project's routines. Each calls the service and answers; the
 * rules and the run lock live in service.ts.
 */
import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { getKey } from '../../app/http.ts';
import { Status } from '../../platform/http-status.ts';
import * as routines from './service.ts';
import { runNow } from './cloud.ts';
import { nextRunAt } from './schedule.ts';
import type { Routine } from './rules.ts';

/**
 * A routine as the routes answer it, with when it runs next (null when it is off).
 * The server works this out from the routine's own time zone, the same way the
 * scheduler decides a run is due, so a client never keeps its own copy of that
 * arithmetic to drift from it.
 */
const withNext = (r: Routine) => ({ ...r, nextRunAt: r.enabled ? nextRunAt(r) : null });

/** Routine routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** GET /routines, the project's routines with their history. */
router.get('/routines', authMiddleware, async (req, res) => {
  res.json({ routines: (await routines.list(getKey(req))).map(withNext) });
});

/** POST /routines, add a routine: name, a prompt or a saved playbook (by name) to play, schedule, enabled. */
router.post('/routines', authMiddleware, async (req, res) => {
  res.status(Status.CREATED).json(withNext(await routines.create(getKey(req), req.body)));
});

/** POST /routines/import, take in routines a desktop kept locally, with their ids and history. */
router.post('/routines/import', authMiddleware, async (req, res) => {
  res.json({ imported: await routines.importRoutines(getKey(req), req.body?.routines) });
});

/** PATCH /routines/:id, change any of name, prompt, playbook (null to ask the prompt again), schedule and enabled; the history is kept. */
router.patch('/routines/:id', authMiddleware, async (req, res) => {
  res.json(withNext(await routines.edit(getKey(req), req.params.id, req.body)));
});

/** DELETE /routines/:id, delete a routine and its history. */
router.delete('/routines/:id', authMiddleware, async (req, res) => {
  await routines.remove(getKey(req), req.params.id);
  res.json({ ok: true });
});

/** POST /routines/:id/claim, claim the routine's next run for one browser; 409 when another has it. */
router.post('/routines/:id/claim', authMiddleware, async (req, res) => {
  res.json(withNext(await routines.claim(getKey(req), req.params.id, req.body || {})));
});

/** PATCH /routines/:id/runs/:runId, record how a run ended. */
router.patch('/routines/:id/runs/:runId', authMiddleware, async (req, res) => {
  res.json(withNext(await routines.finishRun(getKey(req), req.params.id, req.params.runId, req.body)));
});

/** DELETE /routines/:id/runs, clear a routine's finished runs. */
router.delete('/routines/:id/runs', authMiddleware, async (req, res) => {
  res.json(withNext(await routines.clearRuns(getKey(req), req.params.id)));
});

/** POST /routines/:id/run, run a cloud routine now; answers 202 once the run is claimed, its end goes to the history. */
router.post('/routines/:id/run', authMiddleware, async (req, res) => {
  res.status(Status.ACCEPTED).json(withNext(await runNow(getKey(req), req.params.id)));
});
