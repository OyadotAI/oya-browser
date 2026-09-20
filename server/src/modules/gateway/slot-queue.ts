/**
 * Callers waiting for a provider slot, oldest first. A saturated pool queues
 * instead of failing a request that would succeed a second later.
 */

/** One queued caller. */
export interface Waiter {
  /** Whose providers it may use. */
  owner: any;
  /** The strategy it asked for. */
  strategy: any;
  /** Providers it already failed on. */
  exclude: Set<any>;
  /** Hands it a provider (or null) and stops its timer. */
  resolve: (provider: any) => void;
}

/** The waiting line for provider slots. */
export class SlotQueue {
  /** Queued callers, oldest first. */
  private waiters: Waiter[] = [];

  /** Callers waiting. */
  get depth() {
    return this.waiters.length;
  }

  /** Resolves with a provider when one is handed over, or null after `timeoutMs`. */
  wait(timeoutMs, owner, strategy, exclude): Promise<any> {
    return new Promise((resolve) => this.enqueue({ owner, strategy, exclude, resolve: null }, resolve, timeoutMs));
  }

  /** Joins the line with a timer; `resolve` settles the caller's promise. */
  private enqueue(entry: Waiter, resolve, timeoutMs) {
    const timer = setTimeout(() => this.expire(entry, resolve), timeoutMs);
    entry.resolve = (provider) => {
      clearTimeout(timer);
      resolve(provider);
    };
    this.waiters.push(entry);
  }

  /** Timed out: leave the line empty-handed. */
  private expire(entry: Waiter, resolve) {
    this.waiters = this.waiters.filter((w) => w !== entry);
    resolve(null);
  }

  /** Removes and returns the longest waiter that can use this provider, if any. */
  take(provider): Waiter | null {
    const i = this.waiters.findIndex((w) => provider.owner === null || provider.owner === w.owner);
    if (i === -1) return null;
    return this.waiters.splice(i, 1)[0];
  }
}
