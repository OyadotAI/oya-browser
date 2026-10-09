/** Document-scoped native drag transport; an unavailable engine capability never falls back to CDP. */
import type { WebContents } from 'electron';
/** Viewport coordinates supplied by the authorized action boundary. */
export interface DragPoint {
  /** Horizontal position in the owning viewport's logical pixels. */
  x: number;
  /** Vertical position in the owning viewport's logical pixels. */
  y: number;
}
/** Native completion distinguishes an accepted HTML drop from an ordinary held-pointer gesture. */
export type DragKind = 'html' | 'pointer';
/** Only a browser-owned contents exposes the private engine capability. */
export interface NativeDragView {
  /** The engine scopes payloads, hit tests, focus and cancellation to this exact contents/document. */
  webContents: Pick<WebContents, 'isDestroyed'> & {
    /** Resolves after native input/drop acknowledgement; rejects lost ownership or unsupported targets. */
    _dragOya?: (from: DragPoint, to: DragPoint) => Promise<DragKind>;
  };
}
/** Fail before input when the native engine is missing or the requested coordinates are invalid. */
export async function nativeDrag(view: NativeDragView, from: DragPoint, to: DragPoint): Promise<DragKind> {
  const contents = view.webContents;
  if (contents.isDestroyed()) throw new Error('View is destroyed');
  if (![from.x, from.y, to.x, to.y].every(Number.isFinite)) throw new Error('Drag coordinates must be finite');
  if (!contents._dragOya) throw new Error('Unsupported native capability: document-scoped drag');
  const kind = await contents._dragOya(from, to);
  if (kind !== 'html' && kind !== 'pointer') throw new Error('Invalid native drag acknowledgement');
  return kind;
}
