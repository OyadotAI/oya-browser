/** Native frame-tree snapshots use exact frame ownership and document identities, never debugging targets. */
import { randomUUID } from 'node:crypto';
import type { WebFrameMain } from 'electron';
import type { NativePage } from './page.ts';
import { evaluateFrame } from './frames.ts';
/** CDP-shaped frame description assembled from native frame metadata. */
interface FrameDescription {
  /** Browser-owned frame identity, stable while that native frame lives. */
  id: string;
  /** Parent frame identity, omitted on the main frame. */
  parentId?: string;
  /** Isolated document identity, renewed by navigation. */
  loaderId: string;
  /** The native frame name. */
  name: string;
  /** Document URL read together with its identity. */
  url: string;
  /** Native security origin, not parsed from an untrusted URL. */
  securityOrigin: string;
  /** Actual document content type. */
  mimeType: string;
}
/** Recursive shape expected by Page.getFrameTree. */
interface FrameTree {
  /** Description of this exact native frame. */
  frame: FrameDescription;
  /** Descendants from the native frame graph. */
  childFrames?: FrameTree[];
}
/** Native isolated snapshot fields. */
interface FrameDocument {
  /** Opaque identity stored only in this document's isolated world. */
  loaderId: string;
  /** Current document address. */
  url: string;
  /** Current document media type. */
  mimeType: string;
}
/** Bound traversal of untrusted iframe trees. */
const MAX_FRAMES = 128;
/** One connection's frame identities, never derived from active focus or ambiguous URLs. */
export class NativeFrameTree {
  /** Frame wrappers are keys, so a replacement never adopts a disposed frame id. */
  private readonly ids = new WeakMap<WebFrameMain, string>();
  /** Exact frames used by this connection, for isolated marker cleanup. */
  private readonly frames = new Set<WebFrameMain>();
  /** Document marker belongs to this connection's isolated world. */
  private readonly key = `oya-frame-tree-${randomUUID()}`;
  /** Return a consistent native frame graph or fail if its ownership changes during traversal. */
  async snapshot(page: NativePage, target: string): Promise<object> {
    const top = page.webContents.mainFrame;
    const before = [...top.framesInSubtree];
    if (before.length > MAX_FRAMES) throw Error('Native frame-tree limit exceeded');
    const frameTree = await this.tree(top, target);
    const after = top.framesInSubtree;
    if (page.webContents.isDestroyed() || page.webContents.mainFrame !== top || !sameFrames(before, after))
      throw Error('Native frame tree changed during inspection');
    return { frameTree };
  }
  /** Recursively snapshot each exact native child, including cross-origin renderer processes. */
  private async tree(frame: WebFrameMain, id: string, parentId?: string): Promise<FrameTree> {
    this.remember(frame);
    const document = await this.document(frame);
    const description = describeFrame(frame, document, id, parentId);
    const children = await this.children(frame, id);
    const final = await this.document(frame);
    if (final.loaderId !== document.loaderId) throw Error('Native document changed during inspection');
    return { frame: description, ...(children.length ? { childFrames: children } : {}) };
  }
  /** Wait for every native child read to settle before releasing the command gate, even on failure. */
  private async children(frame: WebFrameMain, id: string): Promise<FrameTree[]> {
    const results = await Promise.allSettled(frame.frames.map((child) => this.tree(child, this.id(child), id)));
    const failed = results.find((result) => result.status === 'rejected');
    if (failed?.status === 'rejected') throw failed.reason;
    return results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []));
  }
  /** Bound live wrappers even if a page creates frames while the snapshot is in flight. */
  private remember(frame: WebFrameMain): void {
    for (const known of this.frames) if (known.detached) this.frames.delete(known);
    if (this.frames.size >= MAX_FRAMES && !this.frames.has(frame)) throw Error('Native frame limit exceeded');
    this.frames.add(frame);
  }
  /** Allocate an opaque child-frame id only once for the same native wrapper. */
  id(frame: WebFrameMain): string {
    if (!this.ids.has(frame)) this.ids.set(frame, randomUUID());
    return this.ids.get(frame)!;
  }
  /** Read identity and URL in one bounded native evaluation; an unsupported engine fails explicitly. */
  private async document(frame: WebFrameMain): Promise<FrameDocument> {
    return evaluateFrame(frame, documentScript(this.key)) as Promise<FrameDocument>;
  }
  /** Release only this connection's isolated markers, never page-owned state. */
  dispose(): void {
    for (const frame of this.frames)
      if (!frame.detached) void evaluateFrame(frame, `delete globalThis[${JSON.stringify(this.key)}]`).catch(() => {});
    this.frames.clear();
  }
}
/** Reject added, removed, or replaced frame wrappers, even when their URLs are identical. */
function sameFrames(before: WebFrameMain[], after: WebFrameMain[]): boolean {
  return before.length === after.length && before.every((frame, index) => frame === after[index] && !frame.detached);
}
/** Native frame metadata is never inferred from the currently focused tab. */
function describeFrame(frame: WebFrameMain, document: FrameDocument, id: string, parentId?: string): FrameDescription {
  return { id, ...(parentId ? { parentId } : {}), name: frame.name, securityOrigin: frame.origin, ...document };
}

/** The document marker is read and initialized atomically in the reserved isolated world. */
function documentScript(key: string): string {
  return `(() => {
    const key=${JSON.stringify(key)};
    if (!Object.hasOwn(globalThis,key)) Object.defineProperty(globalThis,key,{value:${JSON.stringify(randomUUID())},configurable:true});
    return {loaderId:globalThis[key],url:document.URL,mimeType:document.contentType};
  })()`;
}
