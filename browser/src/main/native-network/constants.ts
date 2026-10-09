/** Native request control and retained response bounds are independent of page size. */
export const NETWORK_LIMITS = {
  /** Bounded retained request identities per private context. */
  records: 256,
  /** Active interception cannot grow without bound. */
  paused: 32,
  /** Fail closed rather than strand the page after an agent disconnects or fails to answer. */
  pauseMs: 10_000,
  /** Base64 retention budget, separate from the engine's bounded pipe tee. */
  bodyBytes: 16_777_216,
  /** Bounded time waiting for the original pipe to finish, never a refetch. */
  bodyWaitMs: 3_000,
  /** Maximum native filter length. */
  pattern: 256,
  /** Maximum rules per target. */
  patterns: 32,
  /** Native filter input bound protects the browser event loop. */
  url: 32_768,
  /** Shared state-transition budget across all rules per request. */
  matchSteps: 262_144,
} as const;
