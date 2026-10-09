/** Connection-owned, document-scoped DOM reads through native isolated frame execution. */
import { randomUUID } from 'node:crypto';
import type { WebFrameMain } from 'electron';
import type { NativePage } from './page.ts';
import { evaluateFrame } from './frames.ts';
import { inspectionScript } from './inspection-script.ts';
import { INSPECTION } from './inspection-constants.ts';
/** Browser allocation never resets when a renderer navigates, so old ids cannot alias a new document. */
let nextRange = 0;
/** Reserve disjoint ranges across all connections, including ones inspecting the same tab. */
function reserveRange(): number {
  const base = nextRange;
  nextRange += INSPECTION.nodes;
  if (!Number.isSafeInteger(nextRange)) throw Error('Native DOM identity space exhausted');
  return base;
}
/** Release only this inspector's reserved-world registry, without executing in the page world. */
function release(frame: WebFrameMain, key: string): void {
  if (!frame.detached) void evaluateFrame(frame, `delete globalThis[${JSON.stringify(key)}]`).catch(() => {});
}
/** One remote connection owns its registries; no object handles are shared with another connection. */
export class NativeInspection {
  /** Non-page-visible registry key, unique to this connection. */
  private readonly key = `oya-inspect-${randomUUID()}`;
  /** Remember exact frames solely to release their isolated state at disconnect. */
  private readonly frames = new Set<WebFrameMain>();
  /** Execute a bounded read; a new document has no previous registry and rejects stale nodes. */
  async read(page: NativePage, operation: string, params: Record<string, unknown>): Promise<unknown> {
    if (page.webContents.isDestroyed()) throw Error('View is destroyed');
    const frame = page.webContents.mainFrame;
    this.remember(frame);
    const base = reserveRange();
    return evaluateFrame(frame, inspectionScript(this.key, base, operation, params));
  }
  /** Bound live native references and discard detached wrappers between reads. */
  private remember(frame: WebFrameMain): void {
    for (const known of this.frames) if (known.detached) this.frames.delete(known);
    if (!this.frames.has(frame) && this.frames.size >= INSPECTION.frames)
      throw Error('Native inspection frame limit exceeded');
    this.frames.add(frame);
  }
  /** Discard this connection's node handles when its WebSocket closes. */
  dispose(): void {
    for (const frame of this.frames) release(frame, this.key);
    this.frames.clear();
  }
}
