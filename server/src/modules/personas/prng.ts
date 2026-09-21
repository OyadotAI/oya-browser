/**
 * Seeded randomness for fingerprints. The LCG and the string hash are the same
 * as the browser side's, so both derive identical values from one seed.
 */
import { HASH_SHIFT, LCG_INCREMENT, LCG_MULTIPLIER, UINT32_MAX } from './constants.ts';

/** A seeded LCG returning floats in [0, 1]. */
export function createPRNG(seed) {
  let s = seed;
  return function () {
    s = (s * LCG_MULTIPLIER + LCG_INCREMENT) & UINT32_MAX;
    return (s >>> 0) / UINT32_MAX;
  };
}

/** A 32-bit unsigned seed hashed from a string. */
export function seedFromString(str) {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << HASH_SHIFT) - hash + str.charCodeAt(i)) | 0;
  }
  return hash >>> 0;
}

/** One element of `arr`, chosen by `rng`. */
export function pick(arr, rng) {
  return arr[Math.floor(rng() * arr.length)];
}
