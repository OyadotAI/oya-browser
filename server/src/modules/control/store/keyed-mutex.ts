/** A per-key FIFO mutex, so transactions in this process that lock the same row take turns. */

/** Per-key FIFO mutex for transactions in this process. */
export class KeyedMutex {
  /** The promise each key's next waiter queues behind. */
  declare tails: Map<any, any>;
  constructor() {
    this.tails = new Map();
  }
  /** Wait for the key; resolves with the release function and whether anyone was ahead. */
  acquire(key) {
    const prior = this.tails.get(key),
      ahead = prior || Promise.resolve();
    const { promise: held, resolve: release } = Promise.withResolvers();
    this.enqueue(
      key,
      ahead.then(() => held),
    );
    return ahead.then(() => ({ release, waited: !!prior }));
  }
  /** Make `tail` the key's newest waiter, forgetting the key once it is released with nobody behind. */
  private enqueue(key, tail) {
    this.tails.set(key, tail);
    void tail.then(() => {
      if (this.tails.get(key) === tail) this.tails.delete(key);
    });
  }
}
