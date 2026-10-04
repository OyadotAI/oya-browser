/**
 * Target checks before an action: the step's target must match exactly one
 * element. When it does not, a recorded alternative that does is swapped in
 * (auto-heal) and the module is regenerated, within a bounded budget.
 */
import type { Locator, Page } from '@playwright/test';
import { HIDDEN_TARGET_ACTIONS, type Candidate, type Step } from '../workflow/index.ts';
import { REPLAY } from './constants.ts';
import { locate, resolveCandidate, type Scope } from './locate.ts';
import { isRecorded } from './identity.ts';
import type { Run, RunEvent } from './types.ts';

/** Makes the locator for one of the step's candidates. */
type Find = (candidate: Candidate) => Locator;

/** What the run reports when it swaps in a recorded alternative. */
const REPAIRED = 'A recorded alternative uniquely matches the target.';

/** Thrown to make the run regenerate its module after a repair. */
const RECOMPILE = 'RECOMPILE_REPAIRED_STEP';

/** When the step's repair window closes (never, until a repair starts). */
const deadline = (run: Run, id: string): number => run.repairDeadlines.get(id) || Infinity;

/** How long a target check may still take: the step's timeout, cut short by the repair window. */
const remaining = (run: Run, step: Step): number =>
  Math.max(REPLAY.MIN_WAIT_MS, Math.min(step.timeout, deadline(run, step.id) - Date.now()));

/** Counts the locator's matches, giving up when the remaining time runs out. */
function countTarget(run: Run, step: Step, locator: Locator): Promise<number> {
  const wait = remaining(run, step);
  const counting = locator.count();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const fail = () => new Error('Target inspection timed out');
  const expire = new Promise<never>((_, reject) => (timer = setTimeout(() => reject(fail()), wait)));
  return Promise.race([counting, expire]).finally(() => clearTimeout(timer));
}

/** The step's scope (through its frames) and a locator maker with placeholders filled. */
function targeting(run: Run, step: Step, p: Page): Find {
  let scope: Scope = p;
  for (const frame of step.frames) scope = scope.frameLocator(frame);
  // Counted as the generated code acts: among visible elements, except where a target may be hidden.
  const visible = !HIDDEN_TARGET_ACTIONS.includes(step.action);
  return (candidate) => {
    const found = locate(scope, resolveCandidate(candidate, run.vars, run.draft));
    return visible ? found.filter({ visible: true }) : found;
  };
}

/** Whether a locator finds exactly one element, and it is the recorded one. */
async function uniqueRecorded(run: Run, step: Step, locator: Locator): Promise<boolean> {
  return (await countTarget(run, step, locator)) === 1 && isRecorded(locator, step.el, remaining(run, step));
}

/**
 * A target that only appeared after waiting came with a page that was still
 * changing (a search that submits from script a moment after Enter). Its
 * widgets wire up once the page has loaded; acting before that is lost.
 */
async function settleArrived(page: Page): Promise<void> {
  if (typeof page?.waitForLoadState !== 'function') return;
  await page.waitForLoadState('load', { timeout: REPLAY.SETTLE_MS }).catch(() => {});
  await page.waitForLoadState('networkidle', { timeout: REPLAY.SETTLE_MS }).catch(() => {});
}

/**
 * Whether another recorded target already finds exactly one element while the
 * first finds none. The run repairs to it at once rather than wait out the
 * step's whole timeout for a target that was renamed.
 */
async function alternativeReady(run: Run, step: Step, find: Find): Promise<boolean> {
  if (!canRepair(run, step)) return false;
  const unique = async (candidate: Candidate) =>
    (await find(candidate)
      .count()
      .catch(() => 0)) === 1;
  for (const candidate of step.candidates.slice(1)) if (await unique(candidate)) return true;
  return false;
}

/** Matches for the step's first candidate, waiting once for it to attach when there are none and no other target is ready. */
async function countPrimary(run: Run, step: Step, primary: Locator, page: Page, find: Find): Promise<number> {
  let count = await countTarget(run, step, primary);
  if (count || (await alternativeReady(run, step, find))) return count;
  await primary.waitFor({ state: 'attached', timeout: remaining(run, step) }).catch(() => {});
  count = await countTarget(run, step, primary);
  if (count) await settleArrived(page);
  return count;
}

/** The run event describing how many elements matched, and on which page when none did. */
function targetEvent(id: string, count: number, page: Page): RunEvent {
  const message = count === 1 ? 'One matching target' : count ? 'Multiple matching targets' : 'Target not found';
  const url = !count && typeof page?.url === 'function' ? safeUrl(page.url()) : undefined;
  return { kind: 'target', stepId: id, count, message, ...(url ? { url } : {}) };
}

/** A page address without its query or fragment, which can carry tokens. */
function safeUrl(url: string): string {
  return String(url || '').split(/[?#]/)[0];
}

/** Whether the step may still be repaired: auto-heal on, not an assertion, attempts and time left. */
function canRepair(run: Run, step: Step): boolean {
  if (!run.autoHeal || step.action.startsWith('assert_')) return false;
  return (run.attempts.get(step.id) || 0) < REPLAY.MAX_REPAIRS && Date.now() < deadline(run, step.id);
}

/** Puts `candidate` first, tells the parent, and throws so the module is regenerated. */
function applyRepair(run: Run, step: Step, candidate: Candidate): never {
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
 * same element, its id, its name, often still matches exactly one.
 */
async function tryRepair(run: Run, step: Step, find: Find): Promise<void> {
  if (!canRepair(run, step)) return;
  if (!run.repairDeadlines.has(step.id)) run.repairDeadlines.set(step.id, Date.now() + REPLAY.REPAIR_WINDOW_MS);
  for (const candidate of step.candidates.slice(1)) {
    if (Date.now() < deadline(run, step.id) && (await uniqueRecorded(run, step, find(candidate)))) {
      applyRepair(run, step, candidate);
    }
  }
}

/** Why the target check failed: none, or too many, matched. */
function targetError(count: number): Error {
  if (count) return new Error(`${count} elements match. Pick a unique target.`);
  return new Error('Target not found. Pick a replacement or update the wait.');
}

/** Checks the step's target matches exactly one element, repairing it when it can; throws otherwise. */
export async function checkTarget(run: Run, step: Step, p: Page): Promise<void> {
  const find = targeting(run, step, p);
  const primary = find(step.candidates[0]);
  let count = await countPrimary(run, step, primary, p, find);
  // One match that is plainly another element (another tag, a link elsewhere) is no match.
  if (count === 1 && !(await isRecorded(primary, step.el, remaining(run, step)))) count = 0;
  run.emit(targetEvent(step.id, count, p));
  if (count === 1) return;
  await tryRepair(run, step, find);
  throw targetError(count);
}
