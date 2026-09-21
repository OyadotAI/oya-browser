/** Exponential retry delay shared by provider cleanup and webhook delivery. */
import { BACKOFF_BASE_MS, BACKOFF_FACTOR } from './constants.ts';

/** Doubling delay for `attempts` prior tries, doubling at most `maxExponent` times and capped at `capMs`. */
export const backoff = (attempts, maxExponent, capMs) =>
  Math.min(capMs, BACKOFF_BASE_MS * BACKOFF_FACTOR ** Math.min(maxExponent, attempts));
