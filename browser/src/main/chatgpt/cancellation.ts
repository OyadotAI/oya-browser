/** Let local OS waits settle on cancellation even when their underlying API has no abort support. */

/** One abort listener is removed whichever side of the wait completes first. */
interface Cancellation {
  /** Rejects when this attempt is canceled or reaches its deadline. */
  promise: Promise<never>;
  /** Releases the listener after ordinary completion. */
  close(): void;
}
/** A pre-aborted signal must reject without waiting for an event that already happened. */
function cancellation(signal: AbortSignal): Cancellation {
  let cancel = () => {};
  const promise = new Promise<never>((_resolve, reject) => {
    cancel = () => reject(signal.reason);
  });
  signal.addEventListener('abort', cancel, { once: true });
  if (signal.aborted) cancel();
  return { promise, close: () => signal.removeEventListener('abort', cancel) };
}
/** Late OS completion cannot retain an exclusive sign-in slot after the user cancels. */
export function awaitCancelable<T>(work: Promise<T>, signal: AbortSignal): Promise<T> {
  const stopped = cancellation(signal);
  return Promise.race([work, stopped.promise]).finally(stopped.close);
}
