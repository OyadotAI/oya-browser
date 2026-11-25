/** Read-only highlight geometry in the analyzer's isolated world; never modifies the website's DOM. */
import { MAX_ANALYSIS_BOXES } from './constants.ts';
import type { AnalyzedElement } from './narration.ts';

/** A capture pins exact DOM nodes; follow never resolves a reused selector or analysis id to another element. */
type MeasureMode = 'capture' | 'follow' | 'target';

/**
 * Visible, axis-aligned rectangles only. Clip in each document before mapping through
 * frame borders/scales, then into the visual viewport. Unsupported rotated frames
 * omit their highlights rather than lighting an unrelated bounding rectangle.
 * The bounded cache lives only in the browser-owned isolated world and document.
 */
const MEASURE = `(() => {
  const parent = el => el.parentElement || el.getRootNode()?.host;
  const disabled = el => el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true';
  const rect = r => ({ x: r.left, y: r.top, w: r.width, h: r.height });
  const intersect = (a, b, x = true, y = true) => {
    const left = x ? Math.max(a.x, b.x) : a.x, top = y ? Math.max(a.y, b.y) : a.y;
    const right = x ? Math.min(a.x + a.w, b.x + b.w) : a.x + a.w;
    const bottom = y ? Math.min(a.y + a.h, b.y + b.h) : a.y + a.h;
    return { x: left, y: top, w: Math.max(0, right - left), h: Math.max(0, bottom - top) };
  };
  const aligned = style => {
    if (style.rotate && style.rotate !== 'none' && style.rotate !== '0deg') return false;
    if (style.transform === 'none') return true;
    const m = new DOMMatrixReadOnly(style.transform);
    return m.is2D && m.b === 0 && m.c === 0 && m.a > 0 && m.d > 0;
  };
  const client = el => {
    const r = rect(el.getBoundingClientRect());
    const sx = el.offsetWidth ? r.w / el.offsetWidth : 1, sy = el.offsetHeight ? r.h / el.offsetHeight : 1;
    return { x: r.x + el.clientLeft * sx, y: r.y + el.clientTop * sy,
      w: el.clientWidth * sx, h: el.clientHeight * sy, sx, sy };
  };
  const clipped = (el, box) => {
    const win = el.ownerDocument.defaultView;
    if (!el.isConnected || !win || !aligned(win.getComputedStyle(el))) return null;
    if (el.checkVisibility && !el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return null;
    for (let node = parent(el); node; node = parent(node)) {
      const style = win.getComputedStyle(node);
      if (!aligned(style)) return null;
      box = intersect(box, client(node), /hidden|clip|scroll|auto/.test(style.overflowX), /hidden|clip|scroll|auto/.test(style.overflowY));
    }
    return intersect(box, { x: 0, y: 0, w: win.innerWidth, h: win.innerHeight });
  };
  const onTop = (el, box, control = el) => {
    if (!box || !box.w || !box.h) return false;
    const passThrough = disabled(el) && el.ownerDocument.defaultView.getComputedStyle(el).pointerEvents === 'none';
    return [[.5,.5],[.2,.2],[.8,.2],[.2,.8],[.8,.8]].some(([fx, fy]) => {
      const x = box.x + box.w * fx, y = box.y + box.h * fy;
      let node = el;
      while (node) {
        const root = node.getRootNode(), hit = root.elementFromPoint?.(x, y);
        // A disabled, pointer-transparent control hits its painted ancestor.
        // Accept only that ancestry, never an unrelated overlay or sibling.
        // A floating label drawn over its own field focuses that field, so it does not cover it.
        const ownLabel = hit?.closest?.('label')?.control === control;
        if (!hit || (hit !== node && !node.contains(hit) && hit !== control && !ownLabel && !(passThrough && hit.contains(node)))) return false;
        node = root.host;
      }
      return true;
    });
  };
  const place = (el, control = el) => {
    let box = clipped(el, rect(el.getBoundingClientRect()));
    if (!onTop(el, box, control)) return null;
    for (let frame = el.ownerDocument.defaultView.frameElement; frame; frame = frame.ownerDocument.defaultView.frameElement) {
      const area = client(frame);
      box = { x: area.x + box.x * area.sx, y: area.y + box.y * area.sy, w: box.w * area.sx, h: box.h * area.sy };
      box = clipped(frame, intersect(box, area));
      if (!onTop(frame, box)) return null;
    }
    const vv = window.visualViewport || { offsetLeft: 0, offsetTop: 0, scale: 1, width: innerWidth, height: innerHeight };
    box = intersect(box, { x: vv.offsetLeft, y: vv.offsetTop, w: vv.width, h: vv.height });
    return { x: (box.x - vv.offsetLeft) * vv.scale, y: (box.y - vv.offsetTop) * vv.scale, w: box.w * vv.scale, h: box.h * vv.scale };
  };
  const find = (id, selector) => {
    try {
      return selector ? (window.__acQueryShadow?.(selector) || document.querySelector(selector)) : window.__acFindElement?.(id);
    } catch { return null; }
  };
  const mode = __MODE__, key = __KEY__;
  const cache = globalThis.__oyaHighlightMeasure;
  const nodes = mode === 'follow' ? (cache?.key === key ? cache.nodes : [])
    : (__WANTED__).map(([id, type, selector]) => [id, type, find(id, selector)]);
  if (mode !== 'follow') globalThis.__oyaHighlightMeasure = { key, nodes };
  return nodes.flatMap(([id, type, el]) => {
    if (!el?.isConnected) return [];
    try {
      // Styled toggles often paint the associated label while the real input
      // is transparent. Keep the input's identity and measure only its label.
      let box = place(el);
      if (!box && el.tagName === 'INPUT' && /^(radio|checkbox)$/.test(el.type)) {
        for (const label of el.labels || []) {
          if (label.control !== el) continue;
          box = place(label, el);
          if (box?.w > 0 && box?.h > 0) break;
        }
      }
      return box?.w > 0 && box?.h > 0 ? [{ id, type, ...box, ...(disabled(el) && { disabled: true }) }] : [];
    } catch { return []; }
  });
})()`;

/** Builds one native isolated read; follow uses the exact nodes captured under the same random measurement key. */
export function highlightMeasureJs(elements: readonly AnalyzedElement[], mode: MeasureMode, key: string): string {
  const wanted = elements.filter((e) => e.visible).slice(0, MAX_ANALYSIS_BOXES);
  const list = JSON.stringify(wanted.map((e) => [e.id, e.type, e.selector]));
  return MEASURE.replace('__WANTED__', () => list)
    .replace('__MODE__', () => JSON.stringify(mode))
    .replace('__KEY__', () => JSON.stringify(key));
}
