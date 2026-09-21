/**
 * Binds the pointer handlers to the live view, with state that survives renders.
 */
import { useState, type MouseEvent } from 'react';
import { doubleClick, mouseDown, mouseMove, mouseUp, type PointerContext, type PointerState } from './pointer';

/** The frame box's mouse handlers. */
export function usePointer(options: Omit<PointerContext, 'state'>) {
  const [state] = useState<PointerState>(() => ({ down: null, lastMove: 0 }));
  const ctx: PointerContext = { ...options, state };
  return {
    onMouseDown: (e: MouseEvent) => mouseDown(e, ctx),
    onMouseUp: (e: MouseEvent) => mouseUp(e, ctx),
    onDoubleClick: (e: MouseEvent) => doubleClick(e, ctx),
    onMouseMove: (e: MouseEvent) => mouseMove(e, ctx),
  };
}
