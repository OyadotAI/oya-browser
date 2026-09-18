/**
 * Target checks before an action: the step's target must match exactly one
 * element. When it does not, a recorded alternative that does is swapped in
 * (auto-heal) and the module is regenerated, within a bounded budget.
 */
const { REPLAY } = require('../constants.cjs');
const { locate, resolveCandidate } = require('./locate.cjs');

/** What the run reports when it swaps in a recorded alternative. */
const REPAIRED = 'A recorded alternative uniquely matches the target.';

/** Thrown to make the run regenerate its module after a repair. */
const RECOMPILE = 'RECOMPILE_REPAIRED_STEP';

/** When the step's repair window closes (never, until a repair starts). */
const deadline = (run, id) => run.repairDeadlines.get(id) || Infinity;

/** How long a target check may still take: the step's timeout, cut short by the repair window. */
const remaining = (run, step) =>
  Math.max(REPLAY.MIN_WAIT_MS, Math.min(step.timeout, deadline(run, step.id) - Date.now()));

/** Counts the locator's matches, giving up when the remaining time runs out. */
function countTarget(run, step, locator) {
  const wait = remaining(run, step);
  const counting = locator.count();
  let timer;
  const fail = () => new Error('Target inspection timed out');
  const expire = new Promise((_, reject) => (timer = setTimeout(() => reject(fail()), wait)));
  return Promise.race([counting, expire]).finally(() => clearTimeout(timer));
}

/** The step's scope (through its frames) and a locator maker with placeholders filled. */
function targeting(run, step, p) {
  let scope = p;
  for (const frame of step.frames) scope = scope.frameLocator(frame);
  return (candidate) => locate(scope, resolveCandidate(candidate, run.vars, run.draft));
}

/** Matches for the step's first candidate, waiting once for it to attach when there are none. */
async function countPrimary(run, step, primary) {
  let count = await countTarget(run, step, primary);
  if (count) return count;
  await primary.waitFor({ state: 'attached', timeout: remaining(run, step) }).catch(() => {});
  count = await countTarget(run, step, primary);
  return count;
}

/** The run event describing how many elements matched. */
function targetEvent(id, count) {
  const message = count === 1 ? 'One matching target' : count ? 'Multiple matching targets' : 'Target not found';
  return { kind: 'target', stepId: id, count, message };
}

/** Whether the step may still be repaired: auto-heal on, not an assertion, attempts and time left. */
function canRepair(run, step) {
  if (!run.autoHeal || step.action.startsWith('assert_')) return false;
  return (run.attempts.get(step.id) || 0) < REPLAY.MAX_REPAIRS && Date.now() < deadline(run, step.id);
}

/** Puts `candidate` first, tells the parent, and throws so the module is regenerated. */
function applyRepair(run, step, candidate) {
  const id = step.id;
  run.attempts.set(id, (run.attempts.get(id) || 0) + 1);
  const original = step.candidates[0];
  step.candidates = [candidate, ...step.candidates.filter((c) => c !== candidate)];
  run.repairSignal = id;
  run.tell({ type: 'repair', stepId: id, original, replacement: candidate, draft: run.draft });
  run.emit({ kind: 'step', stepId: id, status: 'repairing', message: REPAIRED });
  throw new Error(RECOMPILE);
}

/**
 * Ambiguity is as repairable as absence: another recorded handle for the
 * same element — its id, its name — often still matches exactly one.
 */
async function tryRepair(run, step, find) {
  if (!canRepair(run, step)) return;
  if (!run.repairDeadlines.has(step.id)) run.repairDeadlines.set(step.id, Date.now() + REPLAY.REPAIR_WINDOW_MS);
  for (const candidate of step.candidates.slice(1)) {
    if (Date.now() < deadline(run, step.id) && (await countTarget(run, step, find(candidate))) === 1) {
      applyRepair(run, step, candidate);
    }
  }
}

/** Why the target check failed: none, or too many, matched. */
function targetError(count) {
  if (count) return new Error(`${count} elements match. Pick a unique target.`);
  return new Error('Target not found. Pick a replacement or update the wait.');
}

/** Checks the step's target matches exactly one element, repairing it when it can; throws otherwise. */
async function checkTarget(run, step, p) {
  const find = targeting(run, step, p);
  const count = await countPrimary(run, step, find(step.candidates[0]));
  run.emit(targetEvent(step.id, count));
  if (count === 1) return;
  await tryRepair(run, step, find);
  throw targetError(count);
}

module.exports = { checkTarget };
