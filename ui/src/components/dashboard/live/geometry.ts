/**
 * Displayed → page pixels. The frame is the page at its natural size, drawn
 * with object-contain, so a click is scaled back and the letterbox removed.
 */
import { HALVES } from './constants';
import type { PagePoint } from './types';

/** The box object-contain actually draws the image in: it letterboxes inside the element. */
function drawnBox(el: HTMLImageElement, rect: DOMRect) {
  const scale = Math.min(rect.width / el.naturalWidth, rect.height / el.naturalHeight);
  const offX = rect.left + (rect.width - el.naturalWidth * scale) / HALVES;
  const offY = rect.top + (rect.height - el.naturalHeight * scale) / HALVES;
  return { scale, offX, offY };
}

/** The page point under a screen point, or null when there is no frame yet or it falls in the letterbox. */
export function toPagePoint(el: HTMLImageElement | null, clientX: number, clientY: number): PagePoint | null {
  if (!el || !el.naturalWidth) return null;
  const rect = el.getBoundingClientRect();
  const { scale, offX, offY } = drawnBox(el, rect);
  const x = (clientX - offX) / scale;
  const y = (clientY - offY) / scale;
  if (x < 0 || y < 0 || x > el.naturalWidth || y > el.naturalHeight) return null;
  return { x: Math.round(x), y: Math.round(y), localX: clientX - rect.left, localY: clientY - rect.top };
}
