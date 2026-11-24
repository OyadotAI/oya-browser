/** Opaque references bind an owned native tab, exact frame and isolated document generation. */
import { requireInspectableDebugUrl } from './protected.ts';
import { randomUUID } from 'node:crypto';
import type { WebFrameMain } from 'electron';
import type { NativePage } from '../native/index.ts';
import { BROWSER_DEBUG as C } from '../../shared/browser-debug.ts';
import { debugRequest, requireDebugFrame } from './channel.ts';
/** A document reference includes no URL-based authority. */
export interface DebugDocument {
  /** Opaque external reference. */ ref: string;
  /** Exact browser frame. */ frame: WebFrameMain;
  /** Preload generation. */ document: string;
  /** Observed document URL. */ url: string;
  /** Exact parent association, unavailable only for the main document. */ owner?: DebugOwner;
}
/** One native-authorized parent/child relationship and its private owner node. */
export interface DebugOwner {
  /** Parent document. */ parent: DebugDocument;
  /** Parent-private DOM node ref. */ node: string;
}
/** A globally unique element number resolves only to its exact last-read local node. */
export interface DebugElement {
  /** Owning document. */ document: DebugDocument;
  /** Local node capability. */ local: number;
}
/** One registry belongs to one native tab; closing that tab releases it through a weak key. */
export class DebugRegistry {
  /** Exact immutable tab. */ readonly page: NativePage;
  /** Issued document capabilities, bounded independently of page churn. */ private readonly documents = new Map<
    string,
    DebugDocument
  >();
  /** Current document capability for a native frame wrapper. */ private readonly current = new WeakMap<
    WebFrameMain,
    DebugDocument
  >();
  /** Element numbers from the current framed read. */ private readonly elements = new Map<number, DebugElement>();
  /** Never recycle a number into another element. */ private next = 0;
  /** Bound one ledger to the actual native view. */ constructor(page: NativePage) {
    this.page = page;
  }
  /** Metadata comes from the exact preload, even for duplicate URLs and cross-process frames. */
  async document(frame: WebFrameMain): Promise<DebugDocument> {
    const reply = await debugRequest(this.page, frame, 'metadata');
    const known = this.current.get(frame);
    if (known?.document === reply.document) {
      known.url = String((reply.data as DebugMetadata)?.url ?? known.url);
      return known;
    }
    const value: DebugDocument = {
      ref: `f-${randomUUID()}`,
      frame,
      document: reply.document,
      url: String((reply.data as DebugMetadata)?.url ?? frame.url),
    };
    this.remember(value);
    return value;
  }
  /** A stale id remains stale; cleanup never grants it to a different native document. */
  private remember(value: DebugDocument): void {
    for (const [id, doc] of this.documents)
      if (doc.frame === value.frame || doc.frame.detached || this.current.get(doc.frame) !== doc)
        this.documents.delete(id);
    if (this.documents.size >= C.MAX_FRAMES) throw Error('Frame inventory limit exceeded');
    this.documents.set(value.ref, value);
    this.current.set(value.frame, value);
  }
  /** A caller-supplied URL, selector or other lane's reference cannot resolve here. */
  resolve(ref: string): DebugDocument {
    const value = this.documents.get(ref);
    if (!value) throw Error(`Frame ${ref} changed or closed. Read the page again with frames:true.`);
    requireDebugFrame(this.page, value.frame);
    requireInspectableDebugUrl(value.frame.url);
    return value;
  }
  /** Every operation rechecks its immutable preload generation in the same renderer invocation. */
  async run<T>(doc: DebugDocument, kind: string, params: Record<string, unknown> = {}): Promise<T> {
    return (await debugRequest(this.page, doc.frame, kind, params, doc.document)).data as T;
  }
  /** The parent is armed before the exact native child sends the one-use trusted challenge. */
  async associate(child: DebugDocument, parent: DebugDocument): Promise<DebugOwner> {
    if (child.frame.parent !== parent.frame) throw Error('Frame topology changed');
    const nonce = randomUUID();
    await this.run(parent, 'arm', { nonce });
    await this.run(child, 'challenge', { nonce });
    const node = await this.run<string>(parent, 'owner_result', { nonce });
    await this.run(child, 'metadata');
    await this.run(parent, 'metadata');
    if (child.frame.parent !== parent.frame) throw Error('Frame topology changed');
    return (child.owner = { parent, node });
  }
  /** A new read invalidates all old element capabilities without reusing their numbers. */
  beginRead(): void {
    this.elements.clear();
  }
  /** Publish only bounded positive numbers usable by Keel's existing click/type schemas. */
  element(doc: DebugDocument, local: number): number {
    if (this.elements.size >= C.MAX_FRAMES * C.MAX_ELEMENTS || this.next >= C.MAX_ELEMENT_ID)
      throw Error('Element reference capacity exceeded');
    const id = ++this.next;
    this.elements.set(id, { document: doc, local });
    return id;
  }
  /** Missing references never fall back to another document's analyzer id. */
  target(id: number): DebugElement {
    const value = this.elements.get(id);
    if (!value) throw Error(`Element [${id}] is no longer valid. Read the page again before retrying.`);
    requireDebugFrame(this.page, value.document.frame);
    return value;
  }
}
const registries = new WeakMap<NativePage, DebugRegistry>();
/** Adapter state cannot leak into another lane's native view. */
export function debugRegistry(page: NativePage): DebugRegistry {
  if (!registries.has(page)) registries.set(page, new DebugRegistry(page));
  return registries.get(page)!;
}

/** Metadata comes from the isolated document rather than author-selected frame names. */
interface DebugMetadata {
  /** Observed document URL. */ url?: string;
}
