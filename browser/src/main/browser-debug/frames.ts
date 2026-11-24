/** Native ancestry and authenticated DOM owners make duplicate frame URLs irrelevant. */
import { requireInspectableDebugUrl } from './protected.ts';
import type { WebFrameMain } from 'electron';
import { BROWSER_DEBUG as C } from '../../shared/browser-debug.ts';
import type { DebugDocument, DebugRegistry } from './registry.ts';
/** Coordinates in the current document's CSS viewport. */
export interface DebugPoint {
  /** Horizontal CSS px. */ x: number;
  /** Vertical CSS px. */ y: number;
}
/** Bound the native graph before per-document work can multiply. */
export function debugFrames(registry: DebugRegistry): WebFrameMain[] {
  const frames = registry.page.webContents.mainFrame.framesInSubtree;
  if (frames.length > C.MAX_FRAMES) throw Error('Frame inventory exceeds 128 documents');
  return frames;
}
/** Every owner hop binds a native child and parent document, including identical sibling URLs. */
export async function linkedDocument(registry: DebugRegistry, frame: WebFrameMain, depth = 0): Promise<DebugDocument> {
  requireInspectableDebugUrl(frame.url);
  if (depth > C.MAX_DEPTH) throw Error('Frame nesting exceeds 16 levels');
  const doc = await registry.document(frame);
  if (frame === registry.page.webContents.mainFrame) return doc;
  if (!frame.parent) throw Error('Frame left the owned page');
  const parent = await linkedDocument(registry, frame.parent, depth + 1);
  if (!doc.owner || doc.owner.parent !== parent) await registry.associate(doc, parent);
  return doc;
}
/** Match an actual owner node; native-tree enumeration order never selects a child. */
export async function childForOwner(
  registry: DebugRegistry,
  parent: DebugDocument,
  node: string,
): Promise<DebugDocument> {
  for (const frame of parent.frame.frames) {
    const child = await linkedDocument(registry, frame).catch(() => null);
    if (child?.owner?.parent === parent && child.owner.node === node) return child;
  }
  throw Error('The iframe has no verified live document association');
}
/** Each coordinate projection rechecks the connected owner and parent interception. */
export async function topPoint(
  registry: DebugRegistry,
  doc: DebugDocument,
  point: DebugPoint,
  scroll = false,
  geometry = false,
): Promise<DebugPoint> {
  for (let depth = 0; doc.owner; depth++) {
    if (depth >= C.MAX_DEPTH) throw Error('Frame nesting exceeds 16 levels');
    point = await projectDebugParent(registry, doc, point, scroll, geometry);
    doc = doc.owner.parent;
  }
  return point;
}

/** Recheck the native relation immediately around exact owner projection. */
async function projectDebugParent(
  registry: DebugRegistry,
  doc: DebugDocument,
  point: DebugPoint,
  scroll: boolean,
  geometry: boolean,
): Promise<DebugPoint> {
  const { parent, node } = doc.owner!;
  if (doc.frame.parent !== parent.frame) throw Error('Frame topology changed');
  const next = await registry.run<DebugPoint>(parent, 'project', { ...point, owner: node, scroll, geometry });
  await registry.run(doc, 'metadata');
  return next;
}
