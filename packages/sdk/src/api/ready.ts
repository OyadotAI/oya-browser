/**
 * Waiting for a cloud browser to dial in after `start()`: poll the fleet
 * until it shows up, and stop early when the control plane says its creation
 * has ended.
 */
import { OyaError } from '../errors.js';
import { MS_PER_SECOND, READY_POLL_MS, Status } from '../constants.js';
import type { BrowserInfo, ControlSession } from '../types/index.js';

/** The two reads the wait needs. */
export interface ReadyChecks {
  /** Lists the fleet. */
  list(): Promise<BrowserInfo[]>;
  /** Reads a session's durable state. */
  session(id: string): Promise<ControlSession>;
}

/** Session states from which a browser will never come up. */
const ENDED: ControlSession['state'][] = ['failed', 'stopped', 'unknown_outcome'];

/** Sleeps until the next poll, or until `deadline`, whichever is sooner, so a short budget is not overshot by a whole poll. */
export const pollPause = (deadline: number) => {
  const left = deadline - Date.now();
  return left <= 0 ? Promise.resolve() : new Promise((r) => setTimeout(r, Math.min(READY_POLL_MS, left)));
};

/** The 504 when the browser never dialled in before the budget ran out. */
const notReady = (id: string, timeoutMs: number) =>
  new OyaError(
    `Browser ${id} did not come up within ${Math.round(timeoutMs / MS_PER_SECOND)}s`,
    Status.GATEWAY_TIMEOUT,
    null,
  );

/**
 * Resolves once the browser is connected; throws when it ended or the time ran
 * out. Poll sleeps are capped to the remaining budget, and one last check runs
 * after the deadline so a dial-in during the final pause is not missed.
 */
export async function waitUntilConnected(checks: ReadyChecks, id: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (await isUp(checks, id)) return;
    if (Date.now() >= deadline) throw notReady(id, timeoutMs);
    await pollPause(deadline);
  }
}

/** Whether the browser is in the fleet and alive. */
async function isUp(checks: ReadyChecks, id: string): Promise<boolean> {
  const all = await checks.list();
  if (all.some((b) => b.id === id && b.health !== 'dead')) return true;
  await assertNotEnded(checks, id);
  return false;
}

/** Throws when the session's creation has ended. A session not recorded yet is fine. */
async function assertNotEnded(checks: ReadyChecks, id: string): Promise<void> {
  try {
    const session = await checks.session(id);
    if (ENDED.includes(session.state)) {
      throw new OyaError(`Browser creation ended in ${session.state}`, Status.CONFLICT, session);
    }
  } catch (e) {
    if (!(e instanceof OyaError) || e.status !== Status.NOT_FOUND) throw e;
  }
}
