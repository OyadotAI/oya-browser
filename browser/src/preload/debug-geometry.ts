/** Local DOM geometry samples real hit tests and wrapped client rectangles without page-selected selectors. */
import { BROWSER_DEBUG as C } from '../shared/browser-debug.ts';
/** Viewport CSS coordinates before any native conversion. */
export interface DebugPoint {
  /** Horizontal CSS px. */ x: number;
  /** Vertical CSS px. */ y: number;
}
/** A value-only viewport rectangle. */
export interface DebugRect {
  /** Left. */ x: number;
  /** Top. */ y: number;
  /** Width. */ width: number;
  /** Height. */ height: number;
}
/** Live DOM objects never cross the host boundary. */
export function debugRect(rect: DOMRect): DebugRect {
  return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
}
/** Descend only author-open shadow roots, retaining the actual browser hit. */
export function debugHit(x: number, y: number): Element | null {
  let hit = document.elementFromPoint(x, y);
  for (let depth = 0; hit?.shadowRoot && depth < C.MAX_DEPTH; depth++) {
    const inner = hit.shadowRoot.elementFromPoint(x, y);
    if (!inner || inner === hit) break;
    hit = inner;
  }
  return hit;
}
/** Frame inventory is bounded even when authors create many shadow roots. */
export function debugOwners(): HTMLIFrameElement[] {
  const roots: Array<Document | ShadowRoot> = [document],
    owners: HTMLIFrameElement[] = [];
  for (let index = 0; index < roots.length && index < C.MAX_FRAMES; index++) scanOwners(roots[index], roots, owners);
  return owners;
}
/** Collect one author root without choosing frame identity by its index. */
function scanOwners(
  root: Document | ShadowRoot,
  roots: Array<Document | ShadowRoot>,
  owners: HTMLIFrameElement[],
): void {
  for (const node of root.querySelectorAll('*')) {
    if (node.shadowRoot) roots.push(node.shadowRoot);
    if (node.matches('iframe,frame')) owners.push(node as HTMLIFrameElement);
  }
}
/** A clipped client-rect centre cannot land in the union gap of a wrapped inline link. */
export function debugPoint(node: Element): DebugPoint {
  for (const rect of node.getClientRects()) {
    const point = centre(rect);
    if (point && reaches(node, point)) return point;
  }
  throw Error('The element is covered or outside its viewport. No click was sent; use browser_hit_test.');
}
/** Empty and fully offscreen rectangles do not participate in coverage. */
function centre(rect: DOMRect): DebugPoint | null {
  const left = Math.max(0, rect.left),
    top = Math.max(0, rect.top);
  const right = Math.min(innerWidth, rect.right),
    bottom = Math.min(innerHeight, rect.bottom);
  return right <= left || bottom <= top ? null : { x: (left + right) / C.CENTRE, y: (top + bottom) / C.CENTRE };
}
/** A descendant is reachable; an ancestor or unrelated overlay is not. */
function reaches(node: Element, point: DebugPoint): boolean {
  let hit = debugHit(point.x, point.y);
  for (let depth = 0; hit && depth < C.MAX_DEPTH; depth++) {
    if (hit === node || node.contains(hit)) return true;
    const root = hit.getRootNode();
    hit = root instanceof ShadowRoot ? root.host : null;
  }
  return false;
}
/** Reject transforms for which a scale-only inverse would be misleading. */
function requireAxisAligned(owner: Element): void {
  for (let node: Element | null = owner; node; node = node.parentElement) requireTransform(node);
}
/** Rotation and perspective are explicit partial-coverage boundaries. */
function requireTransform(node: Element): void {
  const transform = getComputedStyle(node).transform;
  if (transform === 'none') return;
  const matrix = new DOMMatrix(transform);
  if (!matrix.is2D || matrix.b || matrix.c || matrix.a <= 0 || matrix.d <= 0)
    throw Error('Rotated or perspective frame geometry is unsupported');
}
/** Convert CSS borders/padding and axis-aligned scale into the exact content origin. */
function frameBox(owner: HTMLIFrameElement) {
  const rect = owner.getBoundingClientRect(),
    style = getComputedStyle(owner);
  const sx = rect.width / owner.offsetWidth,
    sy = rect.height / owner.offsetHeight;
  const left = rect.left + (owner.clientLeft + parseFloat(style.paddingLeft)) * sx;
  const top = rect.top + (owner.clientTop + parseFloat(style.paddingTop)) * sy;
  if (![left, top, sx, sy].every(Number.isFinite) || !sx || !sy) throw Error('Frame geometry unavailable');
  return { sx, sy, left, top };
}
/** Project only a connected owner already authenticated by the native host. */
export function debugProject(owner: HTMLIFrameElement, point: DebugPoint, inverse = false): DebugPoint {
  if (!owner.isConnected) throw Error('Frame owner was detached');
  requireAxisAligned(owner);
  const box = frameBox(owner);
  return inverse
    ? { x: (point.x - box.left) / box.sx, y: (point.y - box.top) / box.sy }
    : { x: box.left + point.x * box.sx, y: box.top + point.y * box.sy };
}
