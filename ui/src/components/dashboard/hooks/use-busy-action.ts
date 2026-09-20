/**
 * The shape most dashboard buttons share: mark busy, do the work, toast a
 * failure, clear busy.
 */
import { useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import type { ShowToast } from '../toast/types';

/** `run(work)` runs `work` with `busy` true around it; a failure becomes an error toast. */
export function useBusyAction() {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const run = (work: () => Promise<void>) => attempt(setBusy, toast, work);
  return { busy, run };
}

/** Runs `work` with busy set around it, toasting a failure. */
async function attempt(setBusy: (busy: boolean) => void, toast: ShowToast, work: () => Promise<void>) {
  setBusy(true);
  try {
    await work();
  } catch (err) {
    toast(errorMessage(err), 'error');
  } finally {
    setBusy(false);
  }
}
