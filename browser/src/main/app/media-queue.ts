/** Serializes native media consent without turning a concurrent, already-authorized request into a denial. */
import { MEDIA_PERMISSION_WAITERS, MEDIA_PERMISSION_WAIT_MS } from './constants.ts';

/** A callback waiting for its own approval; neither decisions nor selected streams are shared. */
interface Waiting {
  /** Rechecks the original requester, document, origin, profile and human ownership. */
  current(): boolean;
  /** Performs only this request's consent after earlier consent settles. */
  prompt(): Promise<boolean>;
  /** Settles this exact native request once. */
  resolve(answer: boolean): void;
  /** Stops waiting without releasing an active native dialog. */
  timer?: ReturnType<typeof setTimeout>;
  /** Only identical document, origin, profile and device requests share a denial. */
  key?: string;
}

/** At most one active approval and a small, time-bounded queue of independent requests. */
export class MediaApprovalQueue {
  /** A native dialog or OS decision is still owned by its original request. */
  private active = false;
  /** FIFO waiters; timed-out requests are removed rather than retaining their documents. */
  private readonly waiting: Waiting[] = [];

  /** Refuse overload, otherwise preserve each request's own document-bound approval. */
  run(current: () => boolean, prompt: () => Promise<boolean>, key?: string): Promise<boolean> {
    if (!this.active) return this.execute(current, prompt, key);
    if (this.waiting.length >= MEDIA_PERMISSION_WAITERS) return Promise.resolve(false);
    return new Promise((resolve) => {
      const item: Waiting = { current, prompt, resolve, key };
      item.timer = setTimeout(() => this.expire(item), MEDIA_PERMISSION_WAIT_MS);
      this.waiting.push(item);
    });
  }

  /** A timeout cancels only a queued request; an active native dialog keeps its ownership. */
  private expire(item: Waiting): void {
    const index = this.waiting.indexOf(item);
    if (index < 0) return;
    this.waiting.splice(index, 1);
    clearTimeout(item.timer);
    item.resolve(false);
  }

  /** Keep the active slot until native consent settles, even when its requester becomes stale. */
  private execute(current: () => boolean, prompt: () => Promise<boolean>, key?: string): Promise<boolean> {
    this.active = true;
    return this.answer(current, prompt)
      .then((answer) => this.decided(answer, key))
      .finally(() => {
        this.active = false;
        this.advance();
      });
  }

  /** Deny settles already-queued duplicates without prompting again; a later explicit retry is independent. */
  private decided(answer: boolean, key?: string): boolean {
    if (!answer && key) for (const item of [...this.waiting]) if (item.key === key) this.expire(item);
    return answer;
  }

  /** Synchronous errors, rejected OS promises and stale requests always fail closed. */
  private async answer(current: () => boolean, prompt: () => Promise<boolean>): Promise<boolean> {
    try {
      return current() && (await prompt()) && current();
    } catch {
      return false;
    }
  }

  /** Transfer the slot before resolving the old caller, so another request cannot overtake the queue. */
  private advance(): void {
    const item = this.waiting.shift();
    if (!item) return;
    clearTimeout(item.timer);
    void this.execute(item.current, item.prompt, item.key).then(item.resolve);
  }
}
