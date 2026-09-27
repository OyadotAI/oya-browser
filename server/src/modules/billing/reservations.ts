/**
 * Cloud browsers admitted but not connected yet. A sandbox takes a minute or
 * two to dial in, and until it does the registry does not show it, so without
 * these a burst of starts would each see nothing running and all pass the cap.
 * A reservation ends when a browser of its owner connects, or after a while.
 *
 * ponytail: in memory on one replica; move to the control store before running
 * more than one server replica.
 */
import { RESERVATION_MS } from './constants.ts';

/** Who a reservation is for: a person, or the whole server for the self-hosted cap. */
export const SERVER = '*';

/** Pending starts by owner, each as when it lapses. */
export class Reservations {
  /** owner -> lapse times, soonest first. */
  declare pending: Map<string, number[]>;
  /** The clock. */
  declare now: () => number;

  /** Reservations kept against `now`. */
  constructor(now: () => number) {
    this.pending = new Map();
    this.now = now;
  }

  /** How many starts `owner` has pending. */
  count(owner: string) {
    const live = (this.pending.get(owner) || []).filter((t) => t > this.now());
    this.pending.set(owner, live);
    return live.length;
  }

  /** Holds `n` places for `owner` until their browsers connect. */
  hold(owner: string, n: number) {
    const lapse = this.now() + RESERVATION_MS;
    this.pending.set(owner, [...(this.pending.get(owner) || []), ...Array(n).fill(lapse)]);
  }

  /** A browser of `owner` connected: it no longer needs its place. */
  release(owner: string) {
    this.pending.get(owner)?.shift();
  }
}
