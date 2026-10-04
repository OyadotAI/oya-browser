/** Human-like timing: randomised pauses and the gap between keystrokes. */
import { TYPE_BASE, WORD_PAUSE, SPECIAL_PAUSE, HESITATION, HESITATION_CHANCE, type Range } from './constants.ts';

/** Resolves after `ms` milliseconds. */
export const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/** A random duration in a `{ base, spread }` range. */
export const jitter = ({ base, spread }: Range): number => base + Math.random() * spread;

/** How long to wait after typing `ch`, given the character before it. */
export function typingDelay(ch: string, prev: string): number {
  let ms = jitter(TYPE_BASE);
  if (prev === ' ') ms += jitter(WORD_PAUSE);
  if (!/[a-zA-Z0-9 ]/.test(ch)) ms += jitter(SPECIAL_PAUSE);
  if (Math.random() < HESITATION_CHANCE) ms += jitter(HESITATION);
  return ms;
}
