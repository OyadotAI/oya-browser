/** A drag leaving the tab strip by a deliberate margin becomes a native-window transfer. */
import { RendererConstants as C } from '../../../core/constants.ts';
/** Synthetic reorder-only strip ports may omit their vertical extent. */
type Edges = Pick<DOMRectReadOnly, 'left' | 'right'> & Partial<Pick<DOMRectReadOnly, 'top' | 'bottom'>>;
/** Only measured vertical bounds allow tear-off; synthetic ports remain reorder-only. */
export function outsideStrip(x: number, y: number, bounds?: Edges): boolean {
  const { top, bottom } = bounds ?? {};
  if (!bounds || top === undefined || bottom === undefined) return false;
  const margin = C.TAB_DRAG_THRESHOLD;
  const horizontal = x < bounds.left - margin || x > bounds.right + margin;
  return horizontal || y < top - margin || y > bottom + margin;
}
