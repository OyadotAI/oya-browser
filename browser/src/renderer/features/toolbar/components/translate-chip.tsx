/**
 * The translate chip in the address bar, beside the bookmark star: one button
 * whose text changes with the active tab's translation (so focus stays on it),
 * and a hidden live line that says when a translation starts, ends or fails.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { Icon, StatusLine } from '../../../ui/index.ts';
import type { TranslateViewModel } from '../view-models/translate-view-model.ts';

/** What the chip is given. */
export interface TranslateChipProps {
  /** The chip's ViewModel. */
  vm: TranslateViewModel;
}

/** The chip, or nothing while there is nothing to offer; the live line is always there, so its first announcement is heard. */
export function TranslateChip({ vm }: TranslateChipProps) {
  const { chip, said } = useViewModel(vm);
  const failed = chip?.tone === 'failed';
  return (
    <>
      {chip && (
        <button
          type="button"
          id="btn-translate"
          className={`translate-chip${chip.tone === 'done' ? ' translated' : ''}${failed ? ' failed' : ''}`}
          title={chip.title}
          aria-describedby={failed ? 'translate-reason' : undefined}
          aria-disabled={chip.action === 'none' || undefined}
          onClick={(event) => (vm.activate(), event.detail && event.currentTarget.blur())}
        >
          <Icon name="translate" />
          <span className="translate-chip-text">{chip.text}</span>
          {failed && (
            <span id="translate-reason" className="sr-only">
              {chip.title}
            </span>
          )}
        </button>
      )}
      <StatusLine live id="translate-status" className="sr-only">
        {said}
      </StatusLine>
    </>
  );
}
