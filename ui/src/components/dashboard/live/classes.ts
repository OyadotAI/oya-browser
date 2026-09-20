/**
 * Class names for the frame and its box, which depend on fit, size and capture.
 */
import type { Fit } from './types';

/** The full-page route fills the viewport, less the page chrome. */
const FULL_PAGE_CAP = 'max-h-[calc(100vh-9rem)]';

/** How tall the frame may grow: the bounded panel window versus the full-page route, which fills the viewport. */
export function sizeCaps(large: boolean) {
  return { fitCap: large ? FULL_PAGE_CAP : 'max-h-[420px]', actualCap: large ? FULL_PAGE_CAP : 'max-h-[70vh]' };
}

/** The focusable box: capped when fitted, scrollable at actual size, ringed while it owns the keyboard. */
export function surfaceClass(fit: Fit, large: boolean, captured: boolean): string {
  const { fitCap, actualCap } = sizeCaps(large);
  const size = fit === 'fit' ? fitCap : `${actualCap} overflow-auto`;
  return `relative select-none outline-none ${size} ${captured ? 'ring-1 ring-inset ring-accent/60' : ''}`;
}

/** The frame image: scaled into the cap when fitted, natural size otherwise. */
export function frameClass(fit: Fit, large: boolean): string {
  return fit === 'fit' ? `block h-auto ${sizeCaps(large).fitCap} w-full object-contain` : 'block max-w-none';
}

/** The hint in the corner: solid while captured, shown on hover otherwise. */
export function hintClass(captured: boolean): string {
  const state = captured
    ? 'border-accent/40 bg-black/70 text-text opacity-100'
    : 'border-border bg-black/60 text-text-muted opacity-0 group-hover:opacity-100';
  return `pointer-events-none absolute bottom-2 left-2 flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] transition-opacity ${state}`;
}
