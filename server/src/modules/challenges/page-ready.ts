/**
 * Reading a page that may still be loading.
 *
 * A detector that runs mid-navigation reads a document the browser has not
 * filled in yet and answers "nothing here". A confirmation loop then takes that
 * silence for a verdict, which is how a password the site refused reported
 * success and a wrong code reported completion. The detectors say `loading`
 * instead, and this is the one place that knows to wait.
 */

import { setTimeout as sleep } from 'timers/promises';
import { PAGE_READY_MS, PAGE_READY_POLL_MS } from './constants.ts';

/**
 * Whether a detector's answer means "the page could not say yet": the detector
 * itself reporting a document still loading, or a script that could not run at
 * all. An answer of `undefined` is a caller with nothing to say, not a page
 * mid-navigation, and waiting on it would hang.
 */
export const loading = (seen) => seen === null || seen?.loading === true;

/** A detector's answer, or null while the page cannot be scripted at all. */
export async function readPage(evaluate, script) {
  try {
    return await evaluate(script);
  } catch {
    return null; /* mid-navigation */
  }
}

/** Runs `script` until the page is able to answer it, or the deadline passes. */
export async function whenReady(evaluate, script, deadline = Date.now() + PAGE_READY_MS) {
  do {
    const seen = await readPage(evaluate, script);
    if (!loading(seen)) return seen;
    if (Date.now() >= deadline) return seen;
    await sleep(PAGE_READY_POLL_MS);
  } while (true);
}
