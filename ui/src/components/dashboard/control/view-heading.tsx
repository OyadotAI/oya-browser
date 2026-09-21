/**
 * The heading over the current Control view: its name and what it is for.
 */
import { VIEWS, VIEW_DESCRIPTIONS } from './constants';
import type { View } from './types';

/** Eyebrow, view name and the view's one-sentence purpose. */
export default function ViewHeading({ view }: { /** The view on screen. */ view: View }) {
  return (
    <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
      <div>
        <p className="eyebrow mb-3 text-text-dim">Workspace control</p>
        <h2 className="text-[28px] font-medium tracking-tight">{VIEWS.find((item) => item.key === view)?.label}</h2>
        <p className="mt-2 max-w-2xl text-[13px] leading-6 text-text-muted">{VIEW_DESCRIPTIONS[view]}</p>
      </div>
    </div>
  );
}
