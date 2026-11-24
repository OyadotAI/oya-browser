/** Actual document hit tests follow verified owners through native frame boundaries. */
import type { DebugDocument, DebugRegistry } from './registry.ts';
import { BROWSER_DEBUG as C } from '../../shared/browser-debug.ts';
import { childForOwner, linkedDocument, topPoint, type DebugPoint } from './frames.ts';
/** Plain DOM geometry confers no host authority. */
interface Bounds extends DebugPoint {
  /** CSS width. */ width: number;
  /** CSS height. */ height: number;
}
/** One local browser hit. */
interface Hit {
  /** Actual element, if any. */ element: null | {
    /** Tag. */ tag: string;
    /** Element id. */ id: string | null;
    /** Class list. */ classes: string[];
    /** Computed pointer-events. */ pointerEvents: string;
    /** Local rectangle. */ bounds: Bounds;
  };
  /** Private owner node. */ owner?: string;
}
/** Traversal retains the original viewport point and exact visited document path. */
interface Traversal {
  /** Registry. */ registry: DebugRegistry;
  /** Current document. */ doc: DebugDocument;
  /** Local point. */ local: DebugPoint;
  /** Top-level point. */ point: DebugPoint;
  /** Visited documents. */ path: Array<{ /** Reference. */ ref: string; /** Observed URL. */ url: string }>;
}
/** No inspection scrolls, focuses or clamps an invalid viewport point. */
export async function debugHitTest(registry: DebugRegistry, point: DebugPoint) {
  const doc = await linkedDocument(registry, registry.page.webContents.mainFrame);
  const walk: Traversal = { registry, doc, local: point, point, path: [] };
  return walkDebugHit(walk);
}
/** Unsupported owner geometry returns the last actual hit with explicit partial coverage. */
async function walkDebugHit(walk: Traversal) {
  for (let depth = 0; depth <= C.MAX_DEPTH; depth++) {
    walk.path.push({ ref: walk.doc.ref, url: walk.doc.url });
    const hit = await walk.registry.run<Hit>(walk.doc, 'hit', { ...walk.local });
    if (!hit.owner || !hit.element) return finishDebugHit(walk, hit);
    try {
      await descendDebugHit(walk, hit.owner);
    } catch (why) {
      return { ...(await finishDebugHit(walk, hit)), partial: true, reason: String((why as Error)?.message ?? why) };
    }
  }
  throw Error('Frame nesting exceeds 16 levels');
}
/** A child is selected by its authenticated exact owner, never by URL or tree position. */
async function descendDebugHit(walk: Traversal, owner: string): Promise<void> {
  const child = await childForOwner(walk.registry, walk.doc, owner);
  walk.local = await walk.registry.run<DebugPoint>(walk.doc, 'project', { ...walk.local, owner, inverse: true });
  walk.doc = child;
}
/** Report both local bounds and correctly scaled top-level bounds. */
async function finishDebugHit(walk: Traversal, hit: Hit) {
  const bounds = hit.element?.bounds;
  const top = bounds ? await debugTopBounds(walk, bounds).catch(() => null) : null;
  const element = hit.element ? { ...hit.element, localBounds: bounds, bounds: top } : null;
  return {
    point: walk.point,
    frame: walk.doc.ref,
    url: walk.doc.url,
    framePath: walk.path,
    element,
    partial: !!bounds && !top,
  };
}
/** Geometry projection does not require a possibly clipped rectangle corner to be hittable. */
async function debugTopBounds(walk: Traversal, b: Bounds): Promise<Bounds> {
  const a = await topPoint(walk.registry, walk.doc, { x: b.x, y: b.y }, false, true);
  const z = await topPoint(walk.registry, walk.doc, { x: b.x + b.width, y: b.y + b.height }, false, true);
  return { ...a, width: z.x - a.x, height: z.y - a.y };
}
