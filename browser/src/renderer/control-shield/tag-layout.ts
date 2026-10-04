/**
 * Where one show's number badges sit, so a badge that would land on another
 * stays hidden rather than piling up unreadably.
 */
import { RendererConstants as C } from '../core/constants.ts';
import { overlap, tucked } from './geometry.ts';
import type { Rect, ShieldBox } from './types.ts';

/** The badges claimed so far in one show. */
export class TagLayout {
  /** Where each shown badge sits. */
  private tags: Rect[] = [];

  /** Whether this box's badge fits without landing on one already shown; if so, it is claimed. */
  claim(box: ShieldBox): boolean {
    const tag = spot(box);
    if (this.tags.some((t) => overlap(tag, t) > 0)) return false;
    this.tags.push(tag);
    return true;
  }

  /** Forgets every badge, for the next show. */
  reset(): void {
    this.tags = [];
  }
}

/** Where a box's badge sits: up and left of its corner, or just inside it at the window's edge; wider per digit. */
function spot(box: ShieldBox): Rect {
  const shift = tucked(box) ? -C.SHIELD_TAG_TUCK_PX : C.SHIELD_TAG_OFFSET_PX;
  const w = C.SHIELD_TAG_PX + C.SHIELD_TAG_DIGIT_PX * (String(box.id).length - 1);
  return { x: box.x - shift, y: box.y - shift, w, h: C.SHIELD_TAG_PX };
}
