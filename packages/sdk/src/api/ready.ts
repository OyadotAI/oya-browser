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

/** Resolves once the browser is connected; throws when it ended or the time ran out. */
export async function waitUntilConnected(checks: ReadyChecks, id: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await isUp(checks, id)) return;
    await new Promise((r) => setTimeout(r, READY_POLL_MS));
  }
  const seconds = Math.round(timeoutMs / MS_PER_SECOND);
  throw new OyaError(`Browser ${id} did not come up within ${seconds}s`, Status.GATEWAY_TIMEOUT, null);
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
