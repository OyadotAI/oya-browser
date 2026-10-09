/** Native retirement attempts every cleanup stage after access is revoked, preserving all failures. */
import type { Session } from 'electron';

/** Isolate each resource failure so later privacy cleanup still runs. */
async function attemptContextCleanup(step: () => void | Promise<void>, failures: unknown[]): Promise<void> {
  try {
    await step();
  } catch (error) {
    failures.push(error);
  }
}
/** Ordered native resources close connections before clearing the private jar and cached responses. */
function contextCleanupSteps(session: Session, close: (session: Session) => void): Array<() => void | Promise<void>> {
  return [
    () => close(session),
    () => session.closeAllConnections(),
    () => session.clearStorageData(),
    () => session.clearCache(),
  ];
}
/** One failed resource must not prevent clearing other private data or closing connections. */
export async function retireContext(session: Session, close: (session: Session) => void): Promise<void> {
  const failures: unknown[] = [];
  for (const step of contextCleanupSteps(session, close)) await attemptContextCleanup(step, failures);
  if (failures.length) throw new AggregateError(failures, 'Native context retirement failed');
}
