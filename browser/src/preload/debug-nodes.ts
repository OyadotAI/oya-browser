/** Per-document node references make reads, hit tests and typing stale-safe without page-selected selectors. */
import { BROWSER_DEBUG as C } from '../shared/browser-debug.ts';
import { debugHit, debugPoint, debugRect } from './debug-geometry.ts';
import {
  debugInteractive,
  debugLabel,
  debugFieldFacts,
  debugControlRole,
  beginDebugSemantics,
  finishDebugSemantics,
} from './debug-controls.ts';
/** Selected controls remain native DOM nodes inside the isolated preload only. */
export class DebugNodes {
  /** A new read invalidates the old nodes; references never retarget matching replacement markup. */ private readonly nodes =
    new Map<number, Element>();
  /** Never reuse a number within a document. */ private next = 0;
  /** Trusted receipt belongs to this exact selected document. */
  private pointer = false;
  /** A new arm removes its preceding acknowledgement listener. */
  private stopPointer?: () => void;
  /** Require native trusted movement before any button press. */
  armPointer(id: number): void {
    this.releasePointer();
    const node = this.get(id);
    this.stopPointer = observeDebugPointer(node, () => {
      this.pointer = true;
    });
  }
  /** Read isolated acknowledgement, never a page-owned status variable. */
  pointerReached(): boolean {
    return this.pointer;
  }
  /** Every terminal path retires the acknowledgement listener. */
  releasePointer(): void {
    this.stopPointer?.();
    this.stopPointer = undefined;
    this.pointer = false;
  }
  /** Read local content only; native main enumerates and labels descendant documents separately. */
  read() {
    this.releasePointer();
    this.nodes.clear();
    beginDebugSemantics();
    try {
      const rows = this.controls().map((node) => this.row(node));
      return {
        title: document.title.slice(0, C.MAX_LABEL),
        url: location.href,
        text: (document.body?.innerText ?? '').slice(0, C.MAX_TEXT),
        elements: rows,
      };
    } finally {
      finishDebugSemantics();
    }
  }
  /** Bounded author controls from light and open shadow roots. */
  private controls(): Element[] {
    const roots: Array<Document | ShadowRoot> = [document],
      found: Element[] = [];
    for (let index = 0; index < roots.length && index < C.MAX_FRAMES; index++) {
      scanDebugControls(roots[index], roots, found);
    }
    return found;
  }
  /** Private node identities retain baseline names and redact password field contents. */
  private row(node: Element) {
    const id = ++this.next;
    this.nodes.set(id, node);
    return {
      id,
      tag: node.localName,
      role: debugControlRole(node),
      name: debugLabel(node),
      facts: debugFieldFacts(node),
      href: node instanceof HTMLAnchorElement ? node.href : null,
      bounds: debugRect(node.getBoundingClientRect()),
      covered: covered(node),
    };
  }
  /** A disconnected node cannot be replaced by a selector with the same id. */
  get(id: number): Element {
    const node = this.nodes.get(id);
    if (!node?.isConnected) throw Error('Element is no longer valid. Read the page again before retrying.');
    return node;
  }
  /** Scroll only for an admitted action, then return a real reachable client-rect centre. */
  point(id: number, scroll: boolean) {
    const node = this.get(id);
    if (scroll) node.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
    return { ...debugPoint(node), bounds: debugRect(node.getBoundingClientRect()) };
  }
  /** Frame typing retains the existing explicit DOM value contract and reports its actual method. */
  type(id: number, text: string, submit: boolean): void {
    const node = this.get(id) as HTMLElement;
    if (node instanceof HTMLSelectElement) return choose(node, text);
    if (node.isContentEditable) node.textContent = text;
    else setInput(node, text);
    node.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    node.dispatchEvent(new Event('change', { bubbles: true }));
    if (submit) (node.closest('form') as HTMLFormElement | null)?.requestSubmit();
  }
}
/** Visibility and coverage are distinct: offscreen controls may be read without being called covered. */
function shown(node: Element): boolean {
  const rect = node.getBoundingClientRect(),
    style = getComputedStyle(node);
  return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
}
/** Only currently visible client rectangles can be classified as covered. */
function covered(node: Element): boolean {
  if (!hasDebugVisibleRect(node)) return false;
  try {
    debugPoint(node);
    return false;
  } catch {
    return true;
  }
}
/** Use native setters so controlled inputs observe the same explicit value update as existing type. */
function setInput(node: HTMLElement, value: string): void {
  if (!(node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement))
    throw Error('Element is not an editable field');
  if (node.disabled || node.readOnly) throw Error('Field is disabled or read-only');
  const prototype = node instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(prototype, 'value')!.set!.call(node, value);
}
/** Select remains an explicit value operation, never a synthetic click reported as native input. */
function choose(node: HTMLSelectElement, value: string): void {
  const match = [...node.options].find(
    (o) => o.value === value || o.text.trim().toLowerCase() === value.trim().toLowerCase(),
  );
  if (!match || node.disabled || match.disabled) throw Error('No enabled option matches that value');
  node.value = match.value;
  node.dispatchEvent(new Event('input', { bubbles: true }));
  node.dispatchEvent(new Event('change', { bubbles: true }));
}
/** Describe the actual hit without invoking page getters or serializing a live node. */
export function debugElement(x: number, y: number) {
  if (![x, y].every(Number.isFinite) || x < 0 || y < 0 || x >= innerWidth || y >= innerHeight)
    throw Error('Point is outside the viewport');
  const node = debugHit(x, y);
  return node ? { node, data: debugElementData(node) } : null;
}
/** Description contains only value data and measured DOM geometry. */
function debugElementData(node: Element) {
  return {
    tag: node.localName,
    id: node.id || null,
    classes: [...node.classList],
    pointerEvents: getComputedStyle(node).pointerEvents,
    bounds: debugRect(node.getBoundingClientRect()),
  };
}
/** An offscreen control is distinct from an intercepted control. */
function hasDebugVisibleRect(node: Element): boolean {
  return [...node.getClientRects()].some(
    (r) => r.right > 0 && r.bottom > 0 && r.left < innerWidth && r.top < innerHeight,
  );
}
/** Bound author roots and interactive output while retaining exact nodes. */
function scanDebugControls(root: Document | ShadowRoot, roots: Array<Document | ShadowRoot>, found: Element[]): void {
  for (const node of root.querySelectorAll('*')) {
    if (node.shadowRoot) roots.push(node.shadowRoot);
    if (found.length < C.MAX_ELEMENTS && debugInteractive(node) && shown(node)) found.push(node);
  }
}

/** A dropped host request cannot retain a pointer listener indefinitely. */
function observeDebugPointer(node: Element, reached: () => void): () => void {
  const listener = (event: MouseEvent) => {
    if (
      event.isTrusted &&
      event.composedPath().some((target) => target instanceof Node && (target === node || node.contains(target)))
    )
      reached();
  };
  const stop = () => {
    document.removeEventListener('mousemove', listener, true);
    clearTimeout(timer);
  };
  const timer = setTimeout(stop, C.OPERATION_MS);
  document.addEventListener('mousemove', listener, true);
  return stop;
}
