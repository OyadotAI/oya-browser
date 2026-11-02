/**
 * The address bar's zoom chip: "110%" beside the star while the shown page is not
 * at actual size, as Chrome shows it; a click (or Enter, Space) puts it back to
 * 100%. A screen reader hears each zoom a person makes, never a tab switch.
 */
import type { MouseEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { ZoomChipViewModel } from '../view-models/zoom-chip-view-model.ts';
import { ZOOM_TEXT } from '../model/constants.ts';

/** What the chip is given. */
export interface ZoomChipProps {
  /** The chip's ViewModel. */
  vm: ZoomChipViewModel;
}

/** Where the keyboard goes once the chip it was on disappears: the star, or Reload without one. */
function focusAfterReset(): void {
  (document.getElementById('btn-bookmark') ?? document.getElementById('btn-reload'))?.focus();
}

/** Resets the zoom; a mouse click lets go of the focus, Enter or Space (a click with no detail) hands it on, so Tab keeps its place. */
function resetFrom(vm: ZoomChipViewModel, event: MouseEvent<HTMLButtonElement>): void {
  vm.reset();
  if (event.detail === 0) focusAfterReset();
  else event.currentTarget.blur();
}

/** The chip, drawn only while zoomed, and its always-present announcement. */
export function ZoomChip({ vm }: ZoomChipProps) {
  const { percent, announcement } = useViewModel(vm);
  return (
    <>
      {vm.shown && (
        <button
          type="button"
          id="btn-zoom"
          className="zoom-chip"
          title={ZOOM_TEXT.title(percent)}
          aria-label={ZOOM_TEXT.label(percent)}
          onClick={(event) => resetFrom(vm, event)}
        >
          {ZOOM_TEXT.chip(percent)}
        </button>
      )}
      <span className="sr-only" role="status" aria-live="polite">
        {announcement}
      </span>
    </>
  );
}
