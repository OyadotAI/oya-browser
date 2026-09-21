/**
 * The live view's command queue. A click must finish before typing, and typing
 * before Enter: parallel HTTP requests can arrive (and finish) in a different
 * order from user input.
 */
import { useCallback, useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import type { Send } from './types';

/** Appends one command to `queue` and returns the new tail, which never rejects. */
function append(queue: Promise<unknown>, mounted: RefObject<boolean>, run: () => Promise<unknown>) {
  return queue.then(() => (mounted.current ? run() : undefined)).catch(() => undefined);
}

/** A ref that is true while the component is mounted, so queued input stops once it is gone. */
function useMounted() {
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => void (mounted.current = false);
  }, []);
  return mounted;
}

/** `send`, but each command waits for the previous one; a failure does not stall the queue. */
export function useInputQueue(send: Send): Send {
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const mounted = useMounted();
  return useCallback(
    (action, params) => (queue.current = append(queue.current, mounted, () => send(action, params))),
    [send, mounted],
  );
}
