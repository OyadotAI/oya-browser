/**
 * One named action at a time: the screens disable the button that is working
 * and report a failure as a toast rather than throwing.
 */
import { useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';

/** Runs `fn`, handing any failure's message to `onError` instead of rejecting. */
async function attempt(fn: () => Promise<unknown>, onError: (message: string) => void) {
  try {
    await fn();
  } catch (err) {
    onError(errorMessage(err));
  }
}

/** Which action is running (null when none), and `run` to start one. A failure is toasted; `run` never rejects. */
export function useBusy() {
  const toast = useToast();
  const [busy, setBusy] = useState<string | null>(null);
  const run = async (what: string, fn: () => Promise<unknown>) => {
    setBusy(what);
    await attempt(fn, (message) => toast(message, 'error'));
    setBusy(null);
  };
  return { busy, run };
}
