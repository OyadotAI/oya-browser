/**
 * Validation run records: starting one, recovering those a crash left
 * running, pruning old ones, and folding worker messages into the record.
 */
const { randomUUID } = require('node:crypto');
const { normalizeDraft, generate } = require('../workflow.cjs');
const { WORKSPACE } = require('../constants.cjs');

/** Statuses of a run that is still going. */
const ACTIVE = ['starting', 'running', 'paused', 'stopping'];

/** Whether a run is still going. */
const isActive = (run) => ACTIVE.includes(run?.status);

/** A fresh run record for the draft, with the exact code it will run. */
function newRun(draft) {
  return {
    ...{ id: randomUUID(), draftId: draft.id, revision: draft.revision, draft: structuredClone(draft) },
    ...{ code: generate(draft).code, status: 'starting', startedAt: Date.now(), events: [], repairs: [] },
  };
}

/** Marks a run Oya closed during as interrupted, and saves it. */
function interrupt(runStore, record) {
  record.run.status = 'interrupted';
  record.run.error =
    'Oya closed during validation. Check the website before retrying; nothing was automatically resubmitted.';
  runStore.save(record);
}

/** Interrupts runs a previous session left going; returns the latest run of `draftId`, if any. */
function recoverRuns(runStore, draftId) {
  let latest = null;
  for (const item of runStore?.list() || []) {
    if (item.error) continue;
    const record = runStore.load(item.id);
    if (isActive(record.run)) interrupt(runStore, record);
    if (!latest && record.run?.draftId === draftId) latest = record.run;
  }
  return latest;
}

/** Whether a stored run is past the count, age or total-size limit. */
const expired = (index, item, total) =>
  index >= WORKSPACE.MAX_RUNS ||
  Date.now() - item.updatedAt > WORKSPACE.RUN_MAX_AGE_MS ||
  total > WORKSPACE.RUN_STORAGE_BYTES;

/** Deletes stored runs past the limits, newest kept first. */
function pruneRuns(runStore) {
  let total = 0;
  for (const [index, item] of (runStore?.list() || []).entries()) {
    if (item.error) continue;
    total += runStore.size(item.id);
    if (expired(index, item, total)) runStore.remove(item.id);
  }
}

/** A run event: keep it (bounded) and follow pause and resume. */
function onEvent(workspace, message) {
  const { run } = workspace;
  run.events.push(message.event);
  if (run.events.length > WORKSPACE.MAX_RUN_EVENTS) run.events.shift();
  if (message.event.status === 'paused') run.status = 'paused';
  if (message.event.status === 'running') run.status = 'running';
}

/** The repaired draft: a new, paused copy named after the original. */
function repairedDraft(draft, message) {
  const name = draft.name.slice(0, WORKSPACE.REPAIR_NAME_LENGTH) + '-repair';
  return normalizeDraft({ ...message.draft, id: randomUUID(), name, repairedFrom: draft.id, phase: 'paused' });
}

/** Saves a repair draft; without secure storage, the run says so instead. */
function saveRepair(workspace, repaired) {
  try {
    workspace.store.save(repaired);
  } catch (e) {
    workspace.storageError = e.message;
    const note = 'Repair could not be saved because secure storage is unavailable.';
    workspace.run.events.push({ kind: 'attention', message: note, at: Date.now() });
  }
}

/** A repair: save it as a new draft beside the original and list it on the run. */
function onRepair(workspace, message) {
  const repaired = repairedDraft(workspace.draft, message);
  saveRepair(workspace, repaired);
  const { stepId, original, replacement } = message;
  workspace.run.repairs.push({ stepId, original, replacement, draftId: repaired.id });
}

/** Worker message type → how it changes the run record. */
const RECEIVE = {
  event: onEvent,
  repair: onRepair,
  finished: (workspace, message) => Object.assign(workspace.run, message, { finishedAt: Date.now() }),
};

/** Folds a worker message into the workspace's run; unknown types change nothing. */
function applyMessage(workspace, message) {
  if (Object.hasOwn(RECEIVE, message.type)) RECEIVE[message.type](workspace, message);
}

module.exports = { isActive, newRun, recoverRuns, pruneRuns, applyMessage };
