/**
 * A time limit on a promise. Validation uses it so a run never waits forever
 * to start, and the recorder so a page that stops answering (an open alert, a
 * hung renderer) cannot wedge every later start, stop and save behind it.
 */

/** `promise`, or a rejection with `message` once `ms` has passed. */
export function withinTime<T>(promise: Promise<T>, ms: number, message?: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const expired = new Promise<never>((_, reject) => (timer = setTimeout(() => reject(new Error(message)), ms)));
  return Promise.race([promise, expired]).finally(() => clearTimeout(timer));
}
