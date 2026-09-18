/**
 * The dialog's save-and-report pattern, in one place.
 */

/** Runs `task` with a busy flag raised around it; a failure goes to `onError` instead of throwing. */
export function withBusy(
  setBusy: (busy: boolean) => void,
  task: () => Promise<unknown>,
  onError: (err: unknown) => void,
) {
  setBusy(true);
  return task()
    .then(() => undefined, onError)
    .finally(() => setBusy(false));
}
