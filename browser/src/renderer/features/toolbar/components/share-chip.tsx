/**
 * The share chip in the toolbar: "● Sharing zoom.us  Go to call", shown on every
 * tab but the shared one and the call's while a call captures a tab. It sits in
 * the toolbar row, so the page under it never moves.
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { ShareChipViewModel } from '../view-models/share-chip-view-model.ts';
import { SHARE_TEXT } from '../model/constants.ts';
import './share-chip.css';

/** What the chip is given. */
export interface ShareChipProps {
  /** The chip's ViewModel. */
  vm: ShareChipViewModel;
}

/** The share chip; not drawn at all while nothing is shared. */
export function ShareChipButton({ vm }: ShareChipProps) {
  const s = useViewModel(vm);
  if (!s.visible) return null;
  return (
    <button className="share-chip" aria-label={s.label} title={s.title} onClick={() => vm.goToCall()}>
      <span className="share-chip-dot" aria-hidden="true"></span>
      <span className="share-chip-status">{s.status}</span>
      <span className="share-chip-action">{SHARE_TEXT.action}</span>
    </button>
  );
}
