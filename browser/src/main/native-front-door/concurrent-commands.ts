/** Bound approved page CDP work without making one unresolved response block other commands. */
import { NATIVE_DOOR } from './constants.ts';

/** Native operations keep their serial queues; the existing page passthrough gets a bounded lane. */
export class ConcurrentCommands {
  /** Underlying work retains its slot until it really settles, even after its transport closes. */
  private pending = 0;

  /** Initiate in arrival order and correlate replies independently rather than ordering completion. */
  run(operation: () => Promise<void>): Promise<void> {
    if (this.pending >= NATIVE_DOOR.maxConcurrent)
      return Promise.reject(Error('Native passthrough concurrency limit exceeded'));
    this.pending++;
    return this.execute(operation).finally(() => this.pending--);
  }
  /** Capture synchronous throws without leaking the admitted slot. */
  private async execute(operation: () => Promise<void>): Promise<void> {
    await operation();
  }
}
