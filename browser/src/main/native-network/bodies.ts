/** Bounded response retention tracks original native streams without refetching or page-visible hooks. */
import { NETWORK_LIMITS } from './constants.ts';
import type { BodyReply } from './types.ts';
/** One retained native stream completion. */
interface BodyRecord {
  /** Promise settles when the native pipe completes or the record is revoked. */
  result: Promise<BodyReply>;
  /** Settler is cleared after the first completion. */
  settle?: (reply: BodyReply) => void;
  /** Retained base64 byte budget. */
  size: number;
}
/** Requests missing from this owner's registry are never resolved through another connection. */
export class NativeBodies {
  /** Captures keyed by this session's native identity. */
  private readonly records = new Map<number, BodyRecord>();
  /** Total retained base64 bytes. */
  private bytes = 0;
  /** Start tracking before the original response can arrive. */
  add(id: number): void {
    this.drop(id);
    let settle!: (reply: BodyReply) => void;
    const result = new Promise<BodyReply>((resolve) => {
      settle = resolve;
    });
    this.records.set(id, { result, settle, size: 0 });
  }
  /** Cap retention without interrupting or changing the page's actual network stream. */
  finish(reply: BodyReply): void {
    const record = this.records.get(reply.id);
    if (!record?.settle) return;
    if (this.bytes + reply.body.length > NETWORK_LIMITS.bodyBytes)
      reply = { id: reply.id, body: '', error: 'Native response retention limit exceeded' };
    record.size = reply.body.length;
    this.bytes += record.size;
    record.settle(reply);
    record.settle = undefined;
  }
  /** An unfinished native stream is bounded by a response deadline, never retried over HTTP. */
  async read(id: number): Promise<object> {
    const record = this.records.get(id);
    if (!record) throw Error('Native response body is unavailable or was evicted');
    const reply = await bodyDeadline(record.result);
    if (this.records.get(id) !== record) throw Error('Native response body was revoked');
    if (reply.error) throw Error(reply.error);
    return { body: reply.body, base64Encoded: true };
  }
  /** Eviction invalidates both retained bytes and already waiting readers. */
  drop(id: number): void {
    const record = this.records.get(id);
    if (!record) return;
    this.bytes -= record.size;
    record.settle?.({ id, body: '', error: 'Native response body was revoked' });
    this.records.delete(id);
  }
  /** Context disposal never retains private response data. */
  clear(): void {
    for (const id of this.records.keys()) this.drop(id);
  }
}
/** Timer cleanup happens for successful, rejected and revoked streams alike. */
function bodyDeadline(result: Promise<BodyReply>): Promise<BodyReply> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(
      () => reject(Error('Original native response body is still pending')),
      NETWORK_LIMITS.bodyWaitMs,
    );
  });
  return Promise.race([result, timeout]).finally(() => clearTimeout(timer));
}
