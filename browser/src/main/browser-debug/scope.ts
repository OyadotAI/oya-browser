/** A finite per-view admission prevents concurrent diagnostic operations and late host continuations. */
import { AsyncLocalStorage } from 'node:async_hooks';
import type { NativePage } from '../native/index.ts';
const OPERATION_MS = 13000;
/** A closed operation cannot borrow a later call's deadline or authorization. */
export interface DebugScope {
  /** Absolute deadline. */ deadline: number;
  /** False after settlement. */ active: boolean;
}
const scopes = new AsyncLocalStorage<DebugScope>();
const busy = new WeakSet<NativePage>();
/** The caller already passed the lane gate; this is serialization, not a second approval mechanism. */
export function withDebugScope<T>(page: NativePage, work: () => Promise<T>): Promise<T> {
  if (busy.has(page)) return Promise.reject(Error('Another browser operation is still pending in this owned tab'));
  const scope = { deadline: Date.now() + OPERATION_MS, active: true };
  busy.add(page);
  return scopes.run(scope, () => finishDebugScope(page, scope, work));
}
/** All terminal paths revoke the exact token rather than a mutable global current operation. */
async function finishDebugScope<T>(page: NativePage, scope: DebugScope, work: () => Promise<T>): Promise<T> {
  try {
    return await work();
  } finally {
    scope.active = false;
    busy.delete(page);
  }
}
/** Pending IPC captures this token before leaving the command's async context. */
export function currentDebugScope(): DebugScope | undefined {
  return scopes.getStore();
}
/** Native input and late IPC replies recheck the captured finite operation. */
export function requireDebugScope(scope = scopes.getStore()): void {
  if (scope && (!scope.active || Date.now() >= scope.deadline))
    throw Error('Browser operation expired. No further native input was sent.');
}
