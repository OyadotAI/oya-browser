/**
 * Waiting for a started cloud browser to dial in. Its own file so the pool's
 * start_browser and a connector's own start share one wait without importing
 * each other.
 */
import { registry } from '../modules/browsers/registry.ts';
import { START_POLL_MS, START_WAIT_MS } from './constants.ts';

/** Cloud browsers dial in after they boot; true once `id` is connected, false after `waitMs` (the start wait). */
export async function connects(id: string, waitMs = START_WAIT_MS) {
  const deadline = Date.now() + waitMs;
  while (!registry.isConnected(id) && Date.now() < deadline) await new Promise((r) => setTimeout(r, START_POLL_MS));
  return registry.isConnected(id);
}
