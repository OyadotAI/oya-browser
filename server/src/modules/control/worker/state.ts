/**
 * The worker's shared state: what health checks read, and the flags that keep
 * each kind of background work from overlapping itself.
 */

/** What the last tick did and what is still outstanding, for health checks. */
export const workerHealth = { lastSuccess: null, lastError: null, pendingCleanup: 0, pendingWebhooks: 0 };

/** In-flight markers: a pass that finds its own flag set returns at once. */
export const flags = {
  /** A tick is running. */
  running: false,
  /** Credentials are being re-checked. */
  validating: false,
  /** Leases are being renewed. */
  heartbeating: false,
  /** The queued-start pass, while one runs. */
  provisioning: null as Promise<void> | null,
  /** The maintenance pass, while one runs. */
  maintenance: null as Promise<void> | null,
  /** When maintenance last started. */
  lastMaintenance: 0,
};

/** The boolean flags `exclusive` can hold. */
type Guard = 'running' | 'validating' | 'heartbeating';

/** Runs `work` unless `guard` is already held, holding it until the work settles. */
export async function exclusive(guard: Guard, work: () => Promise<unknown>) {
  if (flags[guard]) return;
  flags[guard] = true;
  try {
    await work();
  } finally {
    flags[guard] = false;
  }
}

/** Keeps a background failure for the health check. */
export const recordError = (e) => {
  workerHealth.lastError = e.message;
};
